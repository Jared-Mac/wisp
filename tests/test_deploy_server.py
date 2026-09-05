import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, ROOT / filename)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


deploy = module("deploy", "infra/deploy/wisp-deploy-server.py")
gateway = module("gateway", "infra/deploy/wisp-deploy-gateway.py")


class DeploymentTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.state = self.root / "state"
        patcher = patch.object(deploy, "STATE", self.state)
        patcher.start()
        self.addCleanup(patcher.stop)
        self.payload = b"\x7fELFtest server binary"
        self.checksum = hashlib.sha256(self.payload).hexdigest()
        self.commit = "a" * 40

    def stage(self, sequence="1", commit=None, checksum=None, payload=None):
        return deploy.stage(commit or self.commit, checksum or self.checksum, sequence, io.BytesIO(self.payload if payload is None else payload))

    def configured(self, installed=b"old server"):
        binary = self.root / "wisp-server"
        binary.write_bytes(installed)
        backups = self.root / "backups"
        backups.mkdir()
        env = self.root / "server.env"
        env.write_text("WISP_SERVER_ADDR=127.0.0.1:8787\n")
        config = self.root / "config.json"
        config.write_text(json.dumps({"binary": str(binary), "database": str(self.root / "db"), "environment": str(env), "backups": str(backups), "local_health_url": "local", "public_health_url": "public"}))
        patcher = patch.object(deploy, "CONFIG", config)
        patcher.start()
        self.addCleanup(patcher.stop)
        return binary

    def test_gateway_rejects_shell_and_unrelated_commands(self):
        self.assertEqual(gateway.command("status"), ["status"])
        self.assertEqual(gateway.command(f"stage {self.commit} {self.checksum} 42")[0], "stage")
        for value in ["", "bash", "apply", "status; id", "status extra", "scp -t /etc/passwd", "stage ../../bin bad 1"]:
            with self.subTest(value=value), self.assertRaises(ValueError):
                gateway.command(value)

    def test_verified_upload_and_monotonic_queue(self):
        first = self.stage(sequence="2")
        self.assertEqual(first["phase"], "queued")
        self.assertEqual(self.stage(sequence="1", commit="b" * 40)["phase"], "superseded")
        self.assertEqual(self.stage(sequence="2"), first)
        with self.assertRaises(ValueError):
            self.stage(sequence="2", commit="b" * 40)
        self.assertEqual(deploy.read_state()["commit"], self.commit)

    def test_bad_upload_cannot_replace_pending_release(self):
        self.stage()
        for payload in [b"shell script", self.payload + b"corrupt"]:
            with self.assertRaises(ValueError):
                self.stage(sequence="2", payload=payload)
        with patch.object(deploy, "LIMIT", 4), self.assertRaises(ValueError):
            self.stage(sequence="2")
        self.assertEqual((self.state / "pending-server").read_bytes(), self.payload)
        self.assertEqual(list(self.state.glob("upload-*")), [])

    def test_active_voice_defers_without_touching_server(self):
        self.stage()
        binary = self.configured()
        with patch.object(deploy, "rooms_idle", return_value=False), patch.object(deploy, "service") as service, patch.object(deploy, "migration_probe") as probe:
            self.assertEqual(deploy.apply()["phase"], "queued")
            service.assert_not_called()
            probe.assert_not_called()
        self.assertEqual(binary.read_bytes(), b"old server")

    def test_second_voice_check_prevents_restart_after_probe(self):
        self.stage()
        self.configured()
        with patch.object(deploy, "rooms_idle", side_effect=[True, False]), patch.object(deploy, "migration_probe", return_value={"schema": 20}), patch.object(deploy, "inspect_database", return_value={"schema": 20}), patch.object(deploy, "service") as service:
            self.assertEqual(deploy.apply()["phase"], "queued")
            service.assert_not_called()

    def test_same_binary_verifies_without_restarting(self):
        self.stage()
        self.configured(self.payload)
        with patch.object(deploy, "inspect_database", return_value={"schema": 20}), patch.object(deploy, "health") as health, patch.object(deploy, "verify_services"), patch.object(deploy, "verify_running"), patch.object(deploy, "service") as service:
            self.assertEqual(deploy.apply()["phase"], "deployed")
            service.assert_not_called()
            health.assert_called_once_with("public", public=True)
        self.assertFalse((self.state / "pending-server").exists())

    def test_upgrade_backs_up_and_verifies_before_reporting_success(self):
        self.stage()
        binary = self.configured()
        new = {"schema": 21}
        with patch.object(deploy, "rooms_idle", return_value=True), patch.object(deploy, "migration_probe", return_value=new), patch.object(deploy, "inspect_database", side_effect=[{"schema": 20}, new]), patch.object(deploy, "backup_database") as backup, patch.object(deploy, "health"), patch.object(deploy, "verify_services"), patch.object(deploy, "verify_running"), patch.object(deploy, "service") as service:
            result = deploy.apply()
            self.assertEqual(result["phase"], "deployed")
            self.assertEqual(result["schema"], 21)
            self.assertEqual([c.args[0] for c in service.call_args_list], ["stop", "start"])
            backup.assert_called_once()
        self.assertEqual(binary.read_bytes(), self.payload)
        self.assertEqual((Path(result["backup"]) / "wisp-server").read_bytes(), b"old server")

    def test_failure_after_migration_never_restores_database(self):
        self.stage()
        self.configured()
        with patch.object(deploy, "rooms_idle", return_value=True), patch.object(deploy, "migration_probe", return_value={"schema": 21}), patch.object(deploy, "inspect_database", side_effect=[{"schema": 20}, {"schema": 21}]), patch.object(deploy, "backup_database") as backup, patch.object(deploy, "service", side_effect=[None, RuntimeError("start failed")]):
            result = deploy.apply()
            self.assertEqual(result["phase"], "failed")
            self.assertNotIn("binary_rolled_back", result)
            backup.assert_called_once()


if __name__ == "__main__":
    unittest.main()
