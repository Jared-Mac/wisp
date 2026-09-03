import QtQuick
import Quickshell
import Quickshell.Io

Item {
  id: root

  property string clientName: "quickshell"
  readonly property string runtimeDir: Quickshell.env("XDG_RUNTIME_DIR")
  readonly property string configHome: String(Quickshell.env("XDG_CONFIG_HOME")
    || (Quickshell.env("HOME") + "/.config"))
  readonly property string configuredProfile: readConfiguredProfile()
  readonly property string socketPath: runtimeDir + "/wisp/wispd.sock"
  readonly property var activeSocket: socketLoader.item
  readonly property bool daemonConnected: !!(activeSocket && activeSocket.connected)
  property int requestId: 0
  property int reconnectAttempt: 0
  property string lastError: ""
  property var snapshot: ({
    "seq": 0,
    "self": {
      "display_name": root.configuredProfile,
      "presence": "away",
      "connection": "connecting_to_server",
      "muted": false,
      "deafened": false,
      "sharing": false,
      "hangout_id": null,
      "push_to_talk": {
        "enabled": false,
        "active": false,
        "shortcut": null,
        "shortcut_backend": null,
        "shortcut_replaced": []
      },
      "media": {
        "livekit_connected": false,
        "microphone_published": false,
        "received_audio_frames": 0,
        "remote_audio_participants": [],
        "remote_video_participants": [],
        "active_speakers": [],
        "received_video_frames": 0,
        "rendered_video_frames": 0,
        "surface_open": false,
        "surface_error": null,
        "error_code": null,
        "error": null,
        "audio": {
          "input_devices": [],
          "output_devices": [],
          "selected_input_id": null,
          "selected_output_id": null,
          "preset": "clear",
          "input_level": 0,
          "denoiser_active": true,
          "denoiser": "deepfilternet",
          "processing_latency_ms": 30
        },
        "screen_share": {
          "starting": false,
          "active": false,
          "source": null,
          "width": null,
          "height": null,
          "fps": null,
          "published_frames": 0,
          "error": null
        },
        "camera": {
          "devices": [],
          "selected_device_id": null,
          "starting": false,
          "active": false,
          "width": null,
          "height": null,
          "fps": null,
          "published_frames": 0,
          "error": null
        },
        "video": {
          "quality": "high",
          "codec": "h264",
          "available_codecs": ["h264", "vp8", "av1"],
          "encoder_backend": "software",
          "available_encoder_backends": ["software"],
          "hardware_acceleration": false
        },
        "remote_videos": []
      }
    },
    "friends": [],
    "hangouts": [],
    "knocks": []
  })

  readonly property var selfState: snapshot["self"] || ({})
  readonly property var friends: snapshot.friends || []
  readonly property var hangouts: snapshot.hangouts || []
  readonly property var knocks: snapshot.knocks || []
  readonly property var mediaState: selfState.media || ({})
  readonly property var pushToTalkState: selfState.push_to_talk || ({
    "enabled": false,
    "active": false,
    "shortcut": null,
    "shortcut_backend": null,
    "shortcut_replaced": []
  })
  readonly property bool effectiveMuted: !!selfState.muted || !!selfState.deafened
    || (!!pushToTalkState.enabled && !pushToTalkState.active)
  readonly property var activeSpeakers: mediaState.active_speakers || []
  readonly property var remoteVideos: mediaState.remote_videos || []
  readonly property var remoteVideoParticipants: mediaState.remote_video_participants || []
  readonly property bool remoteVideoAvailable: remoteVideos.length > 0
    || remoteVideoParticipants.length > 0
  readonly property string remoteVideoLabel: remoteVideoNames().join(" + ")
  readonly property var audioState: mediaState.audio || ({
    "input_devices": [],
    "output_devices": [],
    "selected_input_id": null,
    "selected_output_id": null,
    "preset": "clear",
    "input_level": 0,
    "denoiser_active": true,
    "denoiser": "deepfilternet",
    "processing_latency_ms": 30
  })
  readonly property var screenShareState: mediaState.screen_share || ({
    "starting": false,
    "active": false,
    "source": null,
    "width": null,
    "height": null,
    "fps": null,
    "published_frames": 0,
    "error": null
  })
  readonly property bool sharing: !!screenShareState.active
  readonly property bool shareStarting: !!screenShareState.starting
  readonly property var cameraState: mediaState.camera || ({
    "devices": [],
    "selected_device_id": null,
    "starting": false,
    "active": false,
    "error": null
  })
  readonly property var videoSettings: mediaState.video || ({
    "quality": "high",
    "codec": "h264",
    "available_codecs": ["h264", "vp8", "av1"],
    "encoder_backend": "software",
    "available_encoder_backends": ["software"],
    "hardware_acceleration": false
  })
  readonly property bool cameraActive: !!cameraState.active
  readonly property bool cameraStarting: !!cameraState.starting
  readonly property bool inHangout: selfState.hangout_id !== null && selfState.hangout_id !== undefined
  readonly property bool hasError: !daemonConnected || selfState.connection === "failed"
    || !!mediaState.error || !!mediaState.surface_error || !!screenShareState.error
    || !!cameraState.error
  readonly property string selfStatusLabel: buildSelfStatusLabel()
  readonly property string errorMessage: !daemonConnected
    ? "wispd is not running"
    : String(lastError || mediaState.error || mediaState.surface_error
      || screenShareState.error || cameraState.error || "")
  readonly property string barText: buildBarText()
  readonly property string barLabel: buildBarLabel()
  readonly property string barTooltip: buildBarTooltip()

  signal commandFailed(string message)

  function readConfiguredProfile() {
    var match = localConfig.text().match(/(?:^|\n)WISP_PROFILE=(Tyler|Jack|Charlie)(?:\n|$)/)
    return match ? String(match[1]) : ""
  }

  FileView {
    id: localConfig
    path: root.configHome + "/wisp/friend.env"
    blockLoading: true
    printErrors: false
    watchChanges: true
    onFileChanged: reload()
  }

  function buildSelfStatusLabel() {
    if (!daemonConnected) return "Disconnected"
    var connection = String(selfState.connection || "offline")
    if (connection === "offline") return "Offline"
    if (connection === "connecting_to_server") return "Connecting"
    if (connection === "joining") return "Joining"
    if (connection === "reconnecting") return "Reconnecting"
    if (connection === "failed") return "Connection failed"
    if (connection === "connected") return "Connected"

    var presence = String(selfState.presence || "away")
    if (presence === "open") return "Open"
    if (presence === "knock") return "Knock first"
    if (presence === "closed") return "Closed"
    if (presence === "away") return "Away"
    return presence.charAt(0).toUpperCase() + presence.slice(1)
  }

  function buildBarText() {
    if (!daemonConnected) return "󰍬  disconnected"
    if (knocks.length > 0) return "󰍬  " + String(knocks[0].from.display_name || "Friend") + " knocked"
    var names = []
    if (inHangout) {
      for (var h = 0; h < hangouts.length; h++) {
        if (hangouts[h].id !== selfState.hangout_id) continue
        var members = hangouts[h].members || []
        for (var m = 0; m < members.length; m++)
          if (members[m].id !== selfState.id) names.push(members[m].display_name)
      }
      var prefix = effectiveMuted ? "󰍭" : "󰍬"
      if (activeSpeakers.length > 0)
        return prefix + "  " + activeSpeakers.join(" + ") + " speaking"
      return prefix + (names.length ? "  " + names.join(" ") : "")
    }
    for (var i = 0; i < friends.length; i++)
      if (friends[i].online) names.push(friends[i].display_name)
    return "󰍬" + (names.length ? "  " + names.join(" ") : "")
  }

  function remoteVideoNames() {
    var names = []
    for (var i = 0; i < remoteVideos.length; i++) {
      var name = String(remoteVideos[i].participant || "")
      if (name && names.indexOf(name) < 0) names.push(name)
    }
    if (!names.length) names = remoteVideoParticipants.slice()
    return names
  }

  function buildBarLabel() {
    if (!daemonConnected) return "disconnected"
    if (knocks.length > 0) return String(knocks[0].from.display_name || "Friend") + " knocked"
    if (sharing && cameraActive) return "screen + camera"
    if (sharing) return "sharing"
    if (cameraActive) return "camera"
    var names = []
    if (inHangout) {
      for (var h = 0; h < hangouts.length; h++) {
        if (hangouts[h].id !== selfState.hangout_id) continue
        var members = hangouts[h].members || []
        for (var m = 0; m < members.length; m++)
          if (members[m].id !== selfState.id) names.push(members[m].display_name)
      }
      if (activeSpeakers.length > 0) return activeSpeakers.join(" + ") + " speaking"
      return names.join(" ")
    }
    for (var i = 0; i < friends.length; i++)
      if (friends[i].online) names.push(friends[i].display_name)
    return names.join(" ")
  }

  function buildBarTooltip() {
    if (hasError) return errorMessage
    var parts = ["Wisp"]
    if (sharing) parts.push("Sharing " + String(screenShareState.source || "screen"))
    if (cameraActive) parts.push("Camera on")
    if (remoteVideoAvailable) parts.push(remoteVideoLabel + " is sharing")
    if (selfState.deafened) parts.push("Deafened")
    else if (effectiveMuted) parts.push("Microphone muted")
    else parts.push("Audio ready")
    if (audioState.denoiser_active) {
      var backend = String(audioState.denoiser || "deepfilternet")
      parts.push(backend === "deepfilternet" ? "DeepFilterNet neural denoiser" : "RNNoise fallback")
    }
    return parts.join(" · ")
  }

  function applySnapshot(next) {
    if (!next) return
    snapshot = next
    var nextSelf = next["self"] || ({})
    var nextMedia = nextSelf.media || ({})
    var nextShare = nextMedia.screen_share || ({})
    var nextCamera = nextMedia.camera || ({})
    lastError = String(nextMedia.error || nextMedia.surface_error
      || nextShare.error || nextCamera.error || "")
  }

  function handleLine(line) {
    var message
    try { message = JSON.parse(line) }
    catch (error) {
      lastError = "Invalid response from wispd"
      return
    }
    if (message.type === "snapshot") {
      applySnapshot(message.snapshot)
      return
    }
    if (message.type === "event" && message.payload && message.payload.snapshot) {
      applySnapshot(message.payload.snapshot)
      return
    }
    if (message.type === "result" && message.ok !== true && message.error) {
      lastError = String(message.error.message || "Wisp command failed")
      commandFailed(lastError)
    }
  }

  function sendThrough(socket, name, args) {
    requestId += 1
    socket.write(JSON.stringify({
      "v": 1,
      "id": "qml-" + requestId,
      "type": "command",
      "name": name,
      "args": args || ({})
    }) + "\n")
    socket.flush()
  }

  function send(name, args) {
    var socket = activeSocket
    if (!socket || !socket.connected) {
      lastError = "wispd is not running"
      commandFailed(lastError)
      return
    }
    sendThrough(socket, name, args)
  }

  function setPresence(value) { send("set_presence", { "presence": value }) }
  function joinFriend(name) { send("join_friend", { "friend": name }) }
  function joinHangout(id) { send("join_hangout", { "hangout_id": id }) }
  function respondKnock(id, response) { send("respond_knock", { "knock_id": id, "response": response }) }
  function toggleMuted() { send("toggle_muted", {}) }
  function toggleDeafened() { send("toggle_deafened", {}) }
  function setPushToTalk(enabled) { send("set_push_to_talk", { "enabled": enabled }) }
  function setPushToTalkShortcut(shortcut) {
    send("set_push_to_talk_shortcut", { "shortcut": shortcut || null })
  }
  function pushToTalkPress() { send("push_to_talk_press", {}) }
  function pushToTalkRelease() { send("push_to_talk_release", {}) }
  function refreshAudioDevices() { send("refresh_audio_devices", {}) }
  function setInputDevice(id) { send("set_input_device", { "id": id }) }
  function setOutputDevice(id) { send("set_output_device", { "id": id }) }
  function setAudioPreset(preset) { send("set_audio_preset", { "preset": preset }) }
  function refreshVideoDevices() { send("refresh_video_devices", {}) }
  function setCameraDevice(id) { send("set_camera_device", { "id": id }) }
  function setVideoQuality(quality) { send("set_video_quality", { "quality": quality }) }
  function setVideoCodec(codec) { send("set_video_codec", { "codec": codec }) }
  function openSurface() { send("open_surface", {}) }
  function closeSurface() { send("close_surface", {}) }
  function toggleSurface() { mediaState.surface_open ? closeSurface() : openSurface() }
  function watchVideo(video, open) {
    if (!video) return
    send("watch_video", {
      "participant": String(video.participant || ""),
      "source": String(video.source || "screen_share"),
      "open": open
    })
  }
  function leave() { send("leave", {}) }
  function toggleShare() {
    if (shareStarting) return
    send("share", { "enabled": !sharing, "source": "portal" })
  }
  function toggleCamera() {
    if (cameraStarting) return
    send("camera", { "enabled": !cameraActive })
  }

  Component {
    id: socketComponent
    Socket {
      id: connection
      path: root.socketPath
      connected: true
      parser: SplitParser {
        splitMarker: "\n"
        onRead: function(line) { root.handleLine(line) }
      }
      onConnectionStateChanged: {
        if (connected) {
          root.reconnectAttempt = 0
          // During Loader construction, this Socket can connect before
          // socketLoader.item points at it. Write through the connected
          // instance so the initial snapshot request cannot be dropped.
          root.sendThrough(connection, "hello", { "client": root.clientName })
          root.sendThrough(connection, "refresh_audio_devices", {})
          root.sendThrough(connection, "refresh_video_devices", {})
        }
      }
    }
  }

  Loader {
    id: socketLoader
    active: true
    sourceComponent: socketComponent
  }

  // Runs only while disconnected. A retry creates a fresh QLocalSocket because
  // reconnecting a failed Quickshell Socket in-place is a no-op.
  Timer {
    interval: Math.min(2000, 200 + root.reconnectAttempt * 150)
    repeat: true
    triggeredOnStart: false
    running: !root.daemonConnected
    onTriggered: {
      root.reconnectAttempt = Math.min(12, root.reconnectAttempt + 1)
      socketLoader.active = false
      socketLoader.active = true
    }
  }
}
