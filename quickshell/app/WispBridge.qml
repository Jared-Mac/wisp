import QtQuick
import Quickshell
import Quickshell.Io
import "ChatLogic.js" as ChatLogic
import "FriendLogic.js" as FriendLogic

Item {
  id: root

  property string clientName: "quickshell"
  readonly property alias workspaceLayout: workspaceLayout
  WispWorkspaceLayout { id: workspaceLayout }
  readonly property alias chatColors: chatColors
  WispChatColors { id: chatColors; conversations: root.conversations }
  readonly property alias friendPreferences: friendPreferences
  WispFriendPreferences {
    id: friendPreferences
    account: String(root.selfState.id || root.configuredProfile || root.selfState.display_name || "")
  }
  // Only the standalone desktop host plays sounds, never its tray adapter.
  property bool notificationSoundsEnabled: false
  property bool appFocused: false
  property bool detachedChatFocused: false
  property bool chatVisible: false
  property bool receivedSnapshot: false
  property string lastReadMessageId: ""
  property alias notificationMuted: notificationSettings.muted
  property alias notificationVolume: notificationSettings.volume
  property alias notificationSoundPath: notificationSettings.soundPath
  property string notificationError: ""
  property var drafts: ({})
  property var pendingAttachments: ({})
  property var importingConversations: ({})
  property var sendingConversations: ({})
  property var savedFiles: ({})
  property var savingFiles: ({})
  property var transferProgress: ({})
  property var chatImageUrls: ({})
  property var imageErrors: ({})
  property var imageRequests: ({})
  property var requests: ({})
  signal clipboardTextReady(string conversationId, string value)
  signal messageMutationFinished(string messageId, string action, bool success, string error)
  signal historyClearFinished(string conversationId, bool success, string error)
  signal roomActionFinished(string action, bool success, string error)
  signal chatCreationFinished(string requestId, bool success, string conversationId, string error)
  signal settingsSaved()
  signal settingsSaveFailed()
  onAppFocusedChanged: markVisibleConversationRead()
  onChatVisibleChanged: markVisibleConversationRead()
  readonly property string runtimeDir: Quickshell.env("XDG_RUNTIME_DIR")
  readonly property string configHome: String(Quickshell.env("XDG_CONFIG_HOME")
    || (Quickshell.env("HOME") + "/.config"))
  readonly property string configuredProfile: readConfiguredProfile()
  readonly property string socketPath: String(Quickshell.env("WISP_SOCKET") || runtimeDir + "/wisp/wispd.sock")
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
        "remote_muted_participants": [],
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
          "source_width": null,
          "source_height": null,
          "width": null,
          "height": null,
          "fps": null,
          "published_frames": 0,
          "encoder_backend": null,
          "viewers": [],
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
          "encoder_backend": null,
          "viewers": [],
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
    "knocks": [],
    "conversations": [],
    "messages": [],
    "spots": [],
    "devices": [],
    "last_invite": null
  })

  readonly property var selfState: snapshot["self"] || ({})
  readonly property var friends: snapshot.friends || []
  readonly property var sortedFriends: FriendLogic.sorted(friends, friendPreferences.favorites)
  readonly property var hangouts: snapshot.hangouts || []
  readonly property var knocks: snapshot.knocks || []
  readonly property var conversations: snapshot.conversations || []
  readonly property var messages: snapshot.messages || []
  readonly property var spots: snapshot.spots || []
  readonly property var devices: snapshot.devices || []
  readonly property var lastInvite: snapshot.last_invite || null
  property string activeConversationId: ""
  property string pendingDirectName: ""
  readonly property var activeConversation: conversationById(activeConversationId)
  readonly property var activeMessages: messagesFor(activeConversationId)
  readonly property int unreadMessages: totalUnread()
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
  readonly property var remoteMutedParticipants: mediaState.remote_muted_participants || []
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
    "source_width": null,
    "source_height": null,
    "width": null,
    "height": null,
    "fps": null,
    "published_frames": 0,
    "encoder_backend": null,
    "viewers": [],
    "error": null
  })
  readonly property string screenSharePreviewUrl: localPreviewUrl(
    "screen-share-preview", Number(screenShareState.published_frames || 0))
  readonly property bool sharing: !!screenShareState.active
  readonly property bool shareStarting: !!screenShareState.starting
  readonly property var cameraState: mediaState.camera || ({
    "devices": [],
    "selected_device_id": null,
    "starting": false,
    "active": false,
    "published_frames": 0,
    "encoder_backend": null,
    "viewers": [],
    "error": null
  })
  readonly property string cameraPreviewUrl: localPreviewUrl(
    "camera-preview", Number(cameraState.published_frames || 0))
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

  function localPreviewUrl(fileStem, revision) {
    if (!runtimeDir || revision < 1) return ""
    return "file://" + runtimeDir + "/wisp/" + fileStem + "-"
      + String(revision % 2) + ".bmp"
  }

  FileView {
    id: localConfig
    path: root.configHome + "/wisp/friend.env"
    blockLoading: true
    printErrors: false
    watchChanges: true
    onFileChanged: reload()
  }

  FileView {
    path: root.configHome + "/wisp/notifications.json"
    blockLoading: true
    blockWrites: true
    atomicWrites: true
    printErrors: false
    watchChanges: true
    onFileChanged: reload()
    onAdapterUpdated: { root.notificationError = ""; writeAdapter() }
    onSaved: root.settingsSaved()
    onSaveFailed: {
      root.notificationError = "Couldn't save notification settings on this device."
      root.settingsSaveFailed()
    }
    JsonAdapter {
      id: notificationSettings
      property bool muted: false
      property int volume: 50
      property string soundPath: ""
    }
  }

  Process {
    id: notificationPlayer
    onExited: function(exitCode, exitStatus) {
      if (exitCode !== 0) root.notificationError = "Could not play this sound. Choose a readable audio file and check that pw-play is installed."
    }
  }

  function playNotificationSound() {
    if (notificationMuted || notificationVolume <= 0 || notificationPlayer.running) return
    notificationError = ""
    var path = String(notificationSoundPath || Qt.resolvedUrl("assets/message.wav"))
    if (path.indexOf("file://") === 0) path = decodeURIComponent(path.slice(7))
    if (path.charAt(0) !== "/") {
      notificationError = "Choose an absolute path to a local audio file."
      return
    }
    notificationPlayer.command = ["pw-play", "--volume",
      String(Math.max(0, Math.min(100, notificationVolume)) / 100), path]
    notificationPlayer.running = true
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
    if (unreadMessages > 0 && !inHangout) return "󰍩  " + String(unreadMessages) + " unread"
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
    if (unreadMessages > 0 && !inHangout) return String(unreadMessages) + " unread"
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
    if (unreadMessages > 0) parts.push(String(unreadMessages) + " unread messages")
    if (sharing) parts.push("Sharing " + String(screenShareState.source || "screen"))
    if (cameraActive) parts.push("Camera on")
    if (remoteVideoAvailable) parts.push(remoteVideoLabel + " is sharing")
    if (remoteMutedParticipants.length > 0)
      parts.push(remoteMutedParticipants.join(" + ") + " muted")
    if (selfState.deafened) parts.push("Deafened")
    else if (effectiveMuted) parts.push("Microphone muted")
    else parts.push("Audio ready")
    if (audioState.denoiser_active) {
      var backend = String(audioState.denoiser || "deepfilternet")
      parts.push(backend === "deepfilternet" ? "DeepFilterNet neural denoiser" : "RNNoise fallback")
    }
    if (inHangout) parts.push(mediaState.e2ee_enabled
      ? "End-to-end encrypted" : "Media encryption off")
    return parts.join(" · ")
  }

  function applySnapshot(next, eventName) {
    if (!next) return
    var incoming = ChatLogic.hasIncomingMessage(receivedSnapshot ? snapshot : null, next, eventName)
    snapshot = next
    receivedSnapshot = true
    if (notificationSoundsEnabled && ChatLogic.shouldPlaySound(incoming,
        appFocused, notificationMuted, notificationVolume)) playNotificationSound()
    var nextSelf = next["self"] || ({})
    var nextMedia = nextSelf.media || ({})
    var nextShare = nextMedia.screen_share || ({})
    var nextCamera = nextMedia.camera || ({})
    lastError = String(nextMedia.error || nextMedia.surface_error
      || nextShare.error || nextCamera.error || "")
    if (pendingDirectName !== "") {
      var wanted = pendingDirectName
      var nextConversations = next.conversations || []
      for (var i = 0; i < nextConversations.length; i++) {
        if (String(nextConversations[i].kind) === "direct"
            && String(nextConversations[i].label) === wanted) {
          activeConversationId = String(nextConversations[i].id)
          pendingDirectName = ""
          send("mark_conversation_read", { "conversation_id": activeConversationId })
          break
        }
      }
    }
    markVisibleConversationRead()
  }

  function markVisibleConversationRead() {
    var c = activeConversation
    if (!appFocused || !chatVisible || !c || !c.last_message || !c.unread_count) return
    var id = String(c.last_message.id)
    if (lastReadMessageId === id) return
    lastReadMessageId = id
    send("mark_conversation_read", { "conversation_id": String(c.id) })
  }

  function conversationById(id) {
    for (var i = 0; i < conversations.length; i++)
      if (String(conversations[i].id) === String(id)) return conversations[i]
    return null
  }

  function messagesFor(id) {
    var result = []
    if (!id) return result
    for (var i = 0; i < messages.length; i++)
      if (String(messages[i].conversation_id) === String(id)) result.push(messages[i])
    return result
  }

  function totalUnread() {
    var count = 0
    for (var i = 0; i < conversations.length; i++)
      count += Number(conversations[i].unread_count || 0)
    return count
  }

  function handleLine(line) {
    var message
    try { message = JSON.parse(line) }
    catch (error) {
      lastError = "Invalid response from wispd"
      return
    }
    if (message.type === "event" && message.name === "file_transfer_progress") {
      var progress = message.payload || ({})
      transferProgress = replaceEntry(transferProgress, progress.direction + ":" + progress.id, progress)
      return
    }
    if (message.type === "snapshot") {
      applySnapshot(message.snapshot)
      return
    }
    if (message.type === "event" && message.payload && message.payload.snapshot) {
      applySnapshot(message.payload.snapshot, message.name)
      return
    }
    if (message.type === "result") finishRequest(message)
    if (message.type === "result" && message.ok !== true && message.error) {
      lastError = String(message.error.message || "Wisp command failed")
      commandFailed(lastError)
    }
  }

  function sendThrough(socket, name, args) {
    requestId += 1
    var id = "qml-" + requestId
    socket.write(JSON.stringify({
      "v": 1,
      "id": id,
      "type": "command",
      "name": name,
      "args": args || ({})
    }) + "\n")
    socket.flush()
    return id
  }

  function send(name, args) {
    var socket = activeSocket
    if (!socket || !socket.connected) {
      lastError = "wispd is not running"
      commandFailed(lastError)
      return
    }
    return sendThrough(socket, name, args)
  }

  function replaceEntry(map, key, value) {
    var next = Object.assign({}, map)
    if (value === undefined) delete next[key]
    else next[key] = value
    return next
  }
  function draftFor(id) { return String(drafts[id] || "") }
  function setDraft(id, value) {
    if (id && draftFor(id) !== value) drafts = replaceEntry(drafts, id, value)
  }
  function pasteClipboard(conversationId) {
    if (!conversationId || sendingConversations[conversationId]) return
    var id = send("paste_clipboard", {})
    if (id) {
      requests[id] = { kind: "paste", conversationId: conversationId }
      importingConversations = replaceEntry(importingConversations, conversationId, (importingConversations[conversationId] || 0) + 1)
    }
  }
  function attachmentsFor(id) { return pendingAttachments[id] || [] }
  function setAttachmentKeep(conversationId, token, keep) {
    pendingAttachments = replaceEntry(pendingAttachments, conversationId, attachmentsFor(conversationId).map(function(a) {
      return a.token === token ? Object.assign({}, a, {keep:keep}) : a
    }))
  }
  function transferLabel(direction, id) {
    var value = transferProgress[direction + ":" + id]
    return value && value.total > 0 ? " " + Math.min(100, Math.floor(value.bytes * 100 / value.total)) + "%" : "…"
  }
  function importChatFiles(conversationId, urls) {
    if (!conversationId || sendingConversations[conversationId]) return
    var values = []
    for (var i = 0; i < urls.length; i++) values.push(String(urls[i]))
    var id = send("import_chat_files", {urls: values})
    if (id) {
      requests[id] = {kind: "import", conversationId: conversationId}
      importingConversations = replaceEntry(importingConversations, conversationId, (importingConversations[conversationId] || 0) + 1)
    }
  }
  function removeAttachment(conversationId, token, alreadySent) {
    if (!alreadySent) send("discard_attachment_draft", {token: token})
    pendingAttachments = replaceEntry(pendingAttachments, conversationId,
      attachmentsFor(conversationId).filter(function(a) { return a.token !== token }))
  }
  function sendAttachmentQueue(conversationId, tokens, caption, originalText) {
    var attachment = attachmentsFor(conversationId).filter(function(a) { return a.token === tokens[0] })[0]
    var id = send("send_attachment_message", {conversation_id: conversationId, token: tokens[0], caption: caption, keep:!!(attachment && attachment.keep)})
    if (id) {
      requests[id] = {kind: "send", conversationId: conversationId, text: originalText, token: tokens[0], remaining: tokens.slice(1)}
      sendingConversations = replaceEntry(sendingConversations, conversationId, true)
    } else sendingConversations = replaceEntry(sendingConversations, conversationId, undefined)
  }
  function sendComposedMessage(conversationId) {
    if (!conversationId || sendingConversations[conversationId] || importingConversations[conversationId]) return
    var text = draftFor(conversationId).trim()
    var attachments = attachmentsFor(conversationId)
    if (attachments.length > 0) {
      sendAttachmentQueue(conversationId, attachments.map(function(a) { return a.token }), text, draftFor(conversationId))
      return
    }
    if (!text) return
    var id = send("send_message", {conversation_id: conversationId, text: text})
    if (id) {
      requests[id] = {kind: "send", conversationId: conversationId, text: draftFor(conversationId), token: ""}
      sendingConversations = replaceEntry(sendingConversations, conversationId, true)
    }
  }
  function saveChatFile(messageId) {
    if (savingFiles[messageId]) return
    var id = send("save_chat_file", {message_id: messageId})
    if (id) {
      requests[id] = {kind: "saveFile", messageId: messageId}
      savingFiles = replaceEntry(savingFiles, messageId, true)
    }
  }
  function fileSize(size) {
    if (size >= 1000000000) return (size / 1000000000).toFixed(2) + " GB"
    return size >= 1048576 ? (size / 1048576).toFixed(1) + " MB"
      : size >= 1024 ? Math.ceil(size / 1024) + " KB" : size + " bytes"
  }
  function loadChatImage(messageId, retry) {
    if (chatImageUrls[messageId] || (imageRequests[messageId] && !retry)) return
    var id = send("load_chat_image", {message_id: messageId})
    if (id) {
      imageRequests[messageId] = true
      requests[id] = {kind: "image", messageId: messageId}
    }
  }
  function finishRequest(message) {
    var action = requests[message.id]
    if (!action) return
    delete requests[message.id]
    var value = message.value || ({})
    var conversationId = action.conversationId
    if (action.kind === "setting") {
      if (message.ok) settingsSaved()
      else settingsSaveFailed()
    } else if (action.kind === "edit" || action.kind === "delete") {
      messageMutationFinished(action.messageId, action.kind, !!message.ok,
        message.error ? String(message.error.message || "Could not update message") : "")
    } else if (action.kind === "clear") {
      historyClearFinished(action.conversationId, !!message.ok, message.error ? String(message.error.message || "Could not clear history") : "")
    } else if (action.kind === "newChat") {
      var created = !!message.ok && !!value.id
      if (created) {
        var next = Object.assign({}, snapshot)
        next.conversations = conversations.filter(function(c) { return String(c.id) !== String(value.id) }).concat([value])
        snapshot = next
      }
      chatCreationFinished(String(message.id), created, created ? String(value.id) : "",
        created ? "" : message.error ? String(message.error.message || "Could not create chat") : "Could not create chat")
    } else if (action.kind === "room") {
      if (message.ok && action.action === "create_room" && value.id) activeConversationId = String(value.id)
      roomActionFinished(action.action, !!message.ok, message.error ? String(message.error.message || "Could not update room") : "")
    } else if (action.kind === "send") {
      sendingConversations = replaceEntry(sendingConversations, conversationId, undefined)
      if (message.ok) {
        if (action.text !== undefined && draftFor(conversationId) === action.text) setDraft(conversationId, "")
        if (action.token) removeAttachment(conversationId, action.token, true)
        if (action.remaining && action.remaining.length > 0)
          sendAttachmentQueue(conversationId, action.remaining, "", undefined)
      }
    } else if (action.kind === "paste" || action.kind === "import") {
      importingConversations = replaceEntry(importingConversations, conversationId,
        Math.max(0, (importingConversations[conversationId] || 1) - 1))
      if (message.ok) {
        var added = value.attachments || (value.token ? [value] : [])
        if (added.length > 0) pendingAttachments = replaceEntry(pendingAttachments, conversationId, attachmentsFor(conversationId).concat(added))
        else if (value.text !== undefined) clipboardTextReady(conversationId, String(value.text))
      }
    } else if (action.kind === "saveFile") {
      savingFiles = replaceEntry(savingFiles, action.messageId, undefined)
      if (message.ok) savedFiles = replaceEntry(savedFiles, action.messageId, value)
    } else if (action.kind === "image") {
      if (message.ok) chatImageUrls = replaceEntry(chatImageUrls, action.messageId, value.url)
      else imageErrors = replaceEntry(imageErrors, action.messageId, true)
    }
  }

  function setPresence(value) { send("set_presence", { "presence": value }) }
  function saveSetting(name, args) {
    var id = send(name, args)
    if (id) requests[id] = {kind: "setting"}
    else settingsSaveFailed()
    return id
  }
  function editChatMessage(messageId, text) {
    var id = send("edit_message", {message_id: messageId, text: text})
    if (id) requests[id] = {kind: "edit", messageId: messageId}
    return !!id
  }
  function deleteChatMessage(messageId) {
    var id = send("delete_message", {message_id: messageId})
    if (id) requests[id] = {kind: "delete", messageId: messageId}
  }
  function joinFriend(name) { send("join_friend", { "friend": name }) }
  function joinHangout(id) { send("join_hangout", { "hangout_id": id }) }
  function joinSpot(id) { send("join_spot", { "spot_id": id }) }
  function openDirect(friendName) {
    pendingDirectName = String(friendName)
    send("open_direct", { "friend": String(friendName) })
  }
  function createChat(group, args) {
    var request = send(group ? "create_group" : "open_direct", args)
    if (request) requests[request] = {kind:"newChat"}
    return request || ""
  }
  function selectConversation(id) {
    activeConversationId = String(id)
    var c = conversationById(id)
    if (c && c.tab_closed) send("set_conversation_tab", { "conversation_id": String(id), "closed": false })
    send("mark_conversation_read", { "conversation_id": activeConversationId })
  }
  function exitConversation(id) {
    send("set_conversation_tab", { "conversation_id": String(id), "closed": true })
  }
  function clearChatHistory(id, forEveryone) {
    var request = send("clear_chat_history", {conversation_id:String(id), for_everyone:!!forEveryone})
    if (request) requests[request] = {kind:"clear",conversationId:String(id)}
    return !!request
  }
  function setFileRetention(id, keep) {
    send("set_file_retention", {message_id:String(id),keep:keep})
  }
  function roomAction(action, args) {
    var request = send(action, args)
    if (request) requests[request] = {kind:"room",action:action}
    return !!request
  }
  function closeConversation() { activeConversationId = "" }
  function sendMessage(text) {
    var trimmed = String(text || "").trim()
    if (!activeConversationId || !trimmed) return false
    send("send_message", {
      "conversation_id": activeConversationId,
      "text": trimmed
    })
    return true
  }
  function createInvite(profile, expiresMinutes) {
    send("create_invite", {
      "profile": String(profile),
      "expires_in_minutes": Number(expiresMinutes || 30)
    })
  }
  function refreshDevices() { send("list_devices", {}) }
  function revokeDevice(id) { send("revoke_device", { "device_id": String(id) }) }
  function respondKnock(id, response) { send("respond_knock", { "knock_id": id, "response": response }) }
  function toggleMuted() { send("toggle_muted", {}) }
  function toggleDeafened() { send("toggle_deafened", {}) }
  function setPushToTalk(enabled) { saveSetting("set_push_to_talk", { "enabled": enabled }) }
  function setPushToTalkShortcut(shortcut) {
    saveSetting("set_push_to_talk_shortcut", { "shortcut": shortcut || null })
  }
  function pushToTalkPress() { send("push_to_talk_press", {}) }
  function pushToTalkRelease() { send("push_to_talk_release", {}) }
  function refreshAudioDevices() { send("refresh_audio_devices", {}) }
  function setInputDevice(id) { saveSetting("set_input_device", { "id": id }) }
  function setOutputDevice(id) { saveSetting("set_output_device", { "id": id }) }
  function setAudioPreset(preset) { saveSetting("set_audio_preset", { "preset": preset }) }
  function refreshVideoDevices() { send("refresh_video_devices", {}) }
  function setCameraDevice(id) { saveSetting("set_camera_device", { "id": id }) }
  function setVideoQuality(quality) { saveSetting("set_video_quality", { "quality": quality }) }
  function setVideoCodec(codec) { saveSetting("set_video_codec", { "codec": codec }) }
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
  function stopCamera() { send("camera", { "enabled": false }) }

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
        } else {
          root.receivedSnapshot = false
          root.requests = ({})
          root.sendingConversations = ({})
          root.importingConversations = ({})
          root.savingFiles = ({})
          root.imageRequests = ({})
          root.lastReadMessageId = ""
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
