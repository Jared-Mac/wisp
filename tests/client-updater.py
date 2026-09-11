"""Isolated updater tests: never contact GitHub or the user's Wisp service."""
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("updater", Path(__file__).resolve().parents[1] / "scripts/wisp-updater.py")
u = importlib.util.module_from_spec(spec)
spec.loader.exec_module(u)


class UpdaterTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.paths = (self.root / "config/wisp", self.root / "state/wisp", self.root / "bin", self.root / "runtime/wisp/updates")
        self.patch = patch.object(u, "roots", return_value=self.paths)
        self.patch.start()
        self.addCleanup(self.patch.stop)
        for path in self.paths:
            path.mkdir(parents=True)
        self.old = {"commit":"a"*40, "commit_time":100}
        u.write(self.paths[0] / "installed-release.json", self.old)
        self.release = dict(format=1, platform="linux-x86_64", commit="b"*40, commit_time=200,
                            archive="wisp-main-linux-x86_64.tar.gz", sha256="c"*64, size=100)

    def test_preferences_are_separate_persistent_and_validated(self):
        self.assertEqual(u.preferences(), u.DEFAULTS)
        u.write(self.paths[0] / "updates.json", dict(automatic=False, check_on_launch=False, background_checks=False, interval_minutes=45))
        self.assertEqual(u.preferences(), dict(automatic=False, check_on_launch=False, background_checks=False, interval_minutes=45))
        u.write(self.paths[0] / "updates.json", dict(automatic="false", interval_minutes=-1))
        self.assertEqual(u.preferences(), u.DEFAULTS)

    def test_checks_are_throttled_manual_check_bypasses_interval(self):
        with patch.object(u, "fetch", return_value=json.dumps(self.release).encode()) as fetch:
            self.assertTrue(u.check()["available"])
            u.check()
            self.assertEqual(fetch.call_count, 1)
            u.write(self.paths[0] / "updates.json", dict(automatic=False, background_checks=False, check_on_launch=False))
            self.assertTrue(u.check(force=True)["available"])
            self.assertEqual(fetch.call_count, 2)

    def test_no_downgrade_from_local_build(self):
        self.assertFalse(u.is_newer(self.release, dict(commit="d"*40, commit_time=300)))
        self.assertFalse(u.is_newer(self.release, self.release))
        self.assertTrue(u.is_newer(self.release, self.old))

    def test_invalid_manifest_and_failed_checks(self):
        for invalid in [[], dict(self.release, archive="../bad"), dict(self.release, size=u.MAX_ARCHIVE+1), dict(self.release, commit="bad"), dict(self.release, sha256=123)]:
            with self.assertRaises(ValueError):
                u.validate_manifest(invalid)
        with patch.object(u, "fetch", side_effect=OSError("private URL")):
            result = u.check(force=True)
        self.assertEqual(result["phase"], "error")
        self.assertNotIn("private URL", result["error"])

    def test_alive_clients_all_must_ack_and_be_idle(self):
        directory = self.paths[3] / "clients"
        good = dict(pid=os.getpid(), start=u.process_start(os.getpid()), safe=True, foreground=False, prepared="fresh")
        u.write(directory / "one.json", good)
        self.assertTrue(u.safe_clients(True, "fresh"))
        self.assertFalse(u.safe_clients(True, "old"))
        u.write(directory / "two.json", dict(good, safe=False))
        self.assertFalse(u.safe_clients(False))
        u.write(directory / "two.json", dict(good, foreground=True))
        self.assertFalse(u.safe_clients(True))
        self.assertTrue(u.safe_clients(False, "fresh"))
        u.write(directory / "two.json", dict(good, start="wrong-process"))
        self.assertTrue(u.safe_clients(True))
        self.assertFalse((directory / "two.json").exists())

    def test_invalid_status_never_counts_as_idle(self):
        for value in [{}, [], {"self":{"connection":"joining"}}, {"self":{"connection":"reconnecting"}}, {"self":{"media":{"livekit_connected":True}}}, {"self":{"hangout_id":"room"}}, {"self":{"media":{"camera":{"active":True}}}}]:
            self.assertFalse(u.idle_snapshot(value))
        self.assertTrue(u.idle_snapshot({"self":{}}))

    def test_archive_cannot_escape_or_create_links(self):
        for name, kind in [("../escape", tarfile.REGTYPE), ("wisp-main-linux-x86_64/link", tarfile.SYMTYPE), ("/absolute", tarfile.REGTYPE)]:
            archive = self.root / "bad.tar.gz"
            with tarfile.open(archive, "w:gz") as tar:
                member=tarfile.TarInfo(name); member.type=kind; member.linkname="/tmp/escape"
                tar.addfile(member, io.BytesIO())
            with self.assertRaises(ValueError):
                u.extract(archive, self.root / "extract")
        self.assertFalse((self.root / "escape").exists())

    def preflight_package(self, account_body):
        package = self.root / "preflight"
        (package / "bin").mkdir(parents=True)
        for name in ["wispd", "wispctl", "wisp-account"]:
            executable = package / "bin" / name
            body = account_body if name == "wisp-account" else "import sys; assert sys.argv[1:] == ['--help']"
            executable.write_text(f"#!{sys.executable}\n{body}\n")
            executable.chmod(0o755)
        return package

    def test_preflight_accepts_legacy_json_account_helper(self):
        package = self.preflight_package("""import sys
assert sys.stdin.read() == ''
sys.stderr.write('Error: parse account request\\n\\nCaused by:\\n    EOF while parsing a value at line 1 column 0\\n')
sys.exit(1)""")
        u.preflight(package)

    def test_preflight_accepts_account_help(self):
        package = self.preflight_package("import sys; assert sys.argv[1:] == ['--help']; print('JSON request on stdin')")
        u.preflight(package)

    def test_preflight_rejects_broken_account_helper(self):
        package = self.preflight_package("import sys; sys.stderr.write('error while loading shared libraries'); sys.exit(127)")
        with self.assertRaises(RuntimeError):
            u.preflight(package)

    def package(self):
        folder = self.root / "wisp-main-linux-x86_64"
        for relative in ["bin/wispd", "bin/wispctl", "bin/wisp-account", "install.sh", "quickshell/app/shell.qml", "quickshell/onboarding/shell.qml"]:
            path=folder / relative; path.parent.mkdir(parents=True, exist_ok=True); path.write_text("new " + relative)
        u.write(folder / "release.json", dict(commit=self.release["commit"],commit_time=200))
        archive = self.root / self.release["archive"]
        with tarfile.open(archive,"w:gz") as tar:
            tar.add(folder,arcname=folder.name)
        self.release.update(size=archive.stat().st_size,sha256=u.digest(archive))
        for name in ["wispd","wispctl","wisp-account"]:
            (self.paths[2] / name).write_text("previous " + name)
        for relative in ["quickshell/wisp/shell.qml", "quickshell/wisp-onboarding/shell.qml"]:
            path=self.paths[0].parent / relative;path.parent.mkdir(parents=True);path.write_text("old UI")
        (self.paths[0] / "account.env").write_text("SECRET-TEST-ACCOUNT")
        u.state(phase="available", available=True, release=self.release)
        return archive

    def worker_run(self, *, failure=False, activity=False, optout=False, corrupt=False, startup_failure=False):
        archive=self.package()
        if corrupt: u.state(release=dict(self.release,sha256="0"*64))
        calls=[]
        snapshots={"self":{"muted":True,"deafened":True,"media":{}}}
        def fetch(url,limit,out):
            out.write(archive.read_bytes())
            if optout: u.write(self.paths[0] / "updates.json", {"automatic":False})
            return archive.stat().st_size
        def run(args, **kwargs):
            args=[str(x) for x in args];calls.append(args)
            if startup_failure and args[0].endswith("wisp-account") and "--help" in args:
                return subprocess.CompletedProcess(args,127,"","fixture startup failure")
            if args[0].endswith("install.sh"):
                self.assertEqual(kwargs["environment"]["WISP_CLIENT_ONLY"],"1")
                for name in ["wispd","wispctl","wisp-account"]:
                    (self.paths[2] / name).write_text("new bin/"+name)
                if failure: raise RuntimeError("fixture installation failure")
                for relative,source in [("quickshell/wisp/shell.qml","app"),("quickshell/wisp-onboarding/shell.qml","onboarding")]:
                    (self.paths[0].parent / relative).write_text("new quickshell/"+source+"/shell.qml")
                u.write(self.paths[0] / "installed-release.json",dict(commit=self.release["commit"],commit_time=200))
            return subprocess.CompletedProcess(args,0,'{"visible":true}',"")
        def safe(automatic,request=None):
            return not (activity and request)
        with patch.object(u,"fetch",side_effect=fetch), patch.object(u,"run",side_effect=run), patch.object(u,"snapshot",return_value=snapshots), patch.object(u,"safe_clients",side_effect=safe), patch.object(u,"verify_daemon"), patch.object(u.time,"sleep"):
            result=u.worker(True)
        self.assertEqual((self.paths[0] / "account.env").read_text(),"SECRET-TEST-ACCOUNT")
        return result,calls

    def test_successful_client_install_verifies_ui_and_preserves_account(self):
        result,calls=self.worker_run()
        self.assertEqual(result["phase"],"updated")
        self.assertTrue(any("install.sh" in a[0] for a in calls))
        self.assertEqual(u.read(self.paths[0] / "installed-release.json")["commit"],self.release["commit"])

    def test_failure_rolls_back_binaries_ui_and_marker(self):
        result,_=self.worker_run(failure=True)
        self.assertEqual(result["phase"],"error")
        self.assertEqual((self.paths[2] / "wispd").read_text(),"previous wispd")
        self.assertEqual((self.paths[0].parent / "quickshell/wisp/shell.qml").read_text(),"old UI")
        self.assertEqual(u.read(self.paths[0] / "installed-release.json"),self.old)
        self.assertEqual(result["request"],"")

    def test_new_activity_after_download_prevents_restart(self):
        result,calls=self.worker_run(activity=True)
        self.assertEqual(result["phase"],"available")
        self.assertFalse(any("stop" in c or "quit" in c or "install.sh" in c[0] for c in calls))

    def test_optout_during_download_prevents_install(self):
        result,calls=self.worker_run(optout=True)
        self.assertEqual(result["phase"],"available")
        self.assertFalse(any("stop" in c or "install.sh" in c[0] for c in calls))

    def test_checksum_mismatch_preserves_running_client(self):
        result,calls=self.worker_run(corrupt=True)
        self.assertEqual(result["phase"],"error")
        self.assertFalse(any("stop" in c or "install.sh" in c[0] for c in calls))

    def test_startup_failure_preserves_running_client(self):
        result,calls=self.worker_run(startup_failure=True)
        self.assertEqual(result["phase"],"error")
        self.assertFalse(any("stop" in c or "quit" in c or "install.sh" in c[0] for c in calls))
        self.assertEqual((self.paths[2] / "wisp-account").read_text(),"previous wisp-account")
        self.assertEqual(u.read(self.paths[0] / "installed-release.json"),self.old)

    def test_dead_worker_unfreezes_ui(self):
        u.state(phase="preparing",worker_pid=os.getpid(),worker_start="old",request="token")
        self.assertEqual(u.status()["request"],"")
        self.assertEqual(u.status()["phase"],"error")


unittest.main()
