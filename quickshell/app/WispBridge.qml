import QtQuick
import Quickshell
import Quickshell.Io
import "ChatLogic.js" as ChatLogic
import "FriendLogic.js" as FriendLogic

Item {
  id: root

  property string clientName: "quickshell"
  readonly property alias workspaceLayout: workspaceLayout
  WispWorkspaceLayout {
    id: workspaceLayout
    onStreamPreferenceSaved: root.settingsSaved()
    onChannelPreferenceSaved: root.settingsSaved()
  }
  readonly property alias chatColors: chatColors
  WispChatColors { id: chatColors; conversations: root.conversations }
  readonly property alias friendPreferences: friendPreferences
  readonly property alias participantVolumes: participantVolumes
  WispParticipantVolumes {
    id: participantVolumes
    account: String((root.voiceServerState.self || {}).id || root.configuredProfile || "")
    onVolumesChanged: root.applyParticipantVolumes()
    onReadyChanged: root.applyParticipantVolumes()
    onSaved: root.settingsSaved()
  }
  property string lastAppliedVolumes: ""
  onDaemonConnectedChanged: { lastAppliedVolumes = ""; if (daemonConnected) { applyParticipantVolumes(); refreshPrivacy() } }
  onVoiceFriendsChanged: applyParticipantVolumes()
  onVoiceHangoutsChanged: applyParticipantVolumes()
  function applyParticipantVolumes() {
    if (!daemonConnected || !participantVolumes.ready) return
    var people = voiceFriends.slice(), values = ({})
    voiceHangouts.forEach(function(room) { people = people.concat(room.members || []) })
    people.forEach(function(person) {
      if (person.id !== root.selfState.id) values[String(person.display_name)] = participantVolumes.volumeFor(person) / 100
    })
    var serialized = JSON.stringify(values)
    if (serialized !== lastAppliedVolumes && send("set_participant_volumes", {volumes: values})) lastAppliedVolumes = serialized
  }
  WispFriendPreferences {
    id: friendPreferences
    account: String(root.selfState.id || root.configuredProfile || root.selfState.display_name || "")
  }
  // Only the standalone desktop host plays sounds, never its tray adapter.
  property bool notificationSoundsEnabled: false
  property bool appFocused: false
  property bool detachedChatFocused: false
  property bool mainWindowOpen: false
  property var mediaTileHost: null
  property bool delegateMediaToDesktop: false
  signal desktopWatchRequested(string participant, string source, bool open)
  property bool delegateConversationsToDesktop: false
  signal desktopConversationTileRequested(string id)
  property var pendingConversationTiles: []
  function openChannel(id, forceNewTile) {
    if (!forceNewTile && !workspaceLayout.channelsAsTiles) { selectConversation(id); return }
    if (delegateConversationsToDesktop) { desktopConversationTileRequested(String(id)); return }
    pendingConversationTiles = pendingConversationTiles.concat([String(id)])
  }
  property var watchedMedia: ({})
  signal mediaWatchReady(var video)
  readonly property string videoSocketPath: socketPath.replace(/\.[^/.]+$/, "") + ".video"
  property int imageViewerSerial: 0
  property var imageViewerFocus: ({})
  readonly property bool imageViewerFocused: Object.keys(imageViewerFocus).length > 0
  function setImageViewerFocus(id, active) {
    imageViewerFocus = replaceEntry(imageViewerFocus, id, active ? true : undefined)
  }
  property bool chatVisible: false
  property int chatFocusSerial: 0
  property var focusedChats: ({})
  readonly property string focusedConversationId: {
    var keys = Object.keys(focusedChats)
    return keys.length ? String(focusedChats[keys[keys.length - 1]]) : ""
  }
  function setChatFocus(key, id) { focusedChats = replaceEntry(focusedChats, key, id || undefined) }
  onFocusedConversationIdChanged: markVisibleConversationRead()
  property bool receivedSnapshot: false
  property string lastReadMessageId: ""
  property alias notificationMuted: notificationSettings.muted
  property alias notificationVolume: notificationSettings.volume
  property alias notificationSoundPath: notificationSettings.soundPath
  property alias notificationPolicy: notificationSettings.policy
  property alias roomNotificationSounds: notificationSettings.roomSounds
  property alias selfRoomNotificationSounds: notificationSettings.selfRoomSounds
  readonly property var eventSoundPaths: notificationSettings.eventSounds
  property var soundQueue: []
  property bool soundPlaybackBusy: false
  function setEventSound(kind,path) { notificationSettings.eventSounds = replaceEntry(notificationSettings.eventSounds,kind,String(path)) }
  readonly property var mutedNotificationChats: JSON.parse(JSON.stringify(notificationSettings.mutedChats))
  function chatNotificationsMuted(id) { return mutedNotificationChats.indexOf(String(id)) >= 0 }
  function toggleChatNotifications(id) {
    if (!id) return
    notificationSettings.mutedChats = chatNotificationsMuted(id)
      ? mutedNotificationChats.filter(function(value) { return value !== String(id) })
      : mutedNotificationChats.concat([String(id)])
  }
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
  property var pendingCreatedConversations: []
  property var privacyStatus: ({configured:false})
  property bool privacyBusy: false
  property string privacyFeedback: ""
  property var serverSettings: ({name:"",role:"",members:[],categories:[],channels:[],rooms:[]})
  property bool serverSettingsBusy: false
  property string serverSettingsFeedback: ""
  readonly property bool canManageServer: !!selfState.server_owner || !!selfState.server_admin
  onCanManageServerChanged: {
    if (canManageServer) refreshServerSettings()
    else {
      serverSettings = ({name:"",role:"",members:[],categories:[],channels:[],rooms:[]})
      serverSettingsFeedback = ""
    }
  }
  function refreshPrivacy() {
    var id = send("privacy_status", {server_id:String(activeServer.id || "")})
    if (id) requests[id] = {kind:"privacyStatus"}
  }
  function configurePrivacy(backup, recovery) {
    var id = send("privacy_enable", {server_id:String(activeServer.id || ""),backup_file:String(backup),recovery_file:String(recovery || "")})
    if (id) { requests[id] = {kind:"privacySetup"}; privacyBusy = true; privacyFeedback = "" }
  }
  function exportPrivacy(backup) {
    var id = send("privacy_export", {server_id:String(activeServer.id || ""),backup_file:String(backup)})
    if (id) { requests[id] = {kind:"privacyExport"}; privacyBusy = true; privacyFeedback = "" }
  }
  function refreshServerSettings() {
    if (!daemonConnected || !canManageServer || serverSettingsBusy) return
    var id = send("server_settings", {server_id:String(activeServer.id || "")})
    if (id) { requests[id] = {kind:"serverSettings"}; serverSettingsBusy = true }
  }
  function serverMutation(action, args) {
    if (!canManageServer || serverSettingsBusy) return false
    var values = Object.assign({}, args || {}, {server_id:String(activeServer.id || "")})
    var id = send(action, values)
    if (id) {
      requests[id] = {kind:"serverMutation",action:action}
      serverSettingsBusy = true
      serverSettingsFeedback = ""
    }
    return !!id
  }
  signal clipboardTextReady(string conversationId, string value)
  signal messageMutationFinished(string messageId, string action, bool success, string error)
  signal historyClearFinished(string conversationId, bool success, string error)
  signal roomActionFinished(string action, bool success, string error)
  signal chatCreationFinished(string requestId, bool success, string conversationId, string error)
  signal imageCopyFinished(string requestId, bool success, string error)
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
          "processing_latency_ms": 30,
          "deepfilter_strength": 100,
          "processing_time_us": 0,
          "processing_deadline_misses": 0,
          "capture_queue_ms": 0
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

  function scopedConversationId(serverId, conversationId) {
    return String(serverId) + "::" + String(conversationId)
  }
  function scopedConversation(server, conversation) {
    var value = Object.assign({}, conversation)
    value.raw_id = String(conversation.id)
    value.server_id = String(server.id)
    value.server_name = String(server.name)
    value.id = scopedConversationId(server.id, conversation.id)
    return value
  }
  function scopedMessage(server, message) {
    var value = Object.assign({}, message)
    value.server_id = String(server.id)
    value.server_name = String(server.name)
    value.raw_conversation_id = String(message.conversation_id)
    value.conversation_id = scopedConversationId(server.id, message.conversation_id)
    return value
  }
  function flattenedSnapshot(value) {
    if (!value) return {conversations:[],messages:[],room_invitations:[]}
    var states=(value.server_states || []).length ? value.server_states : [{
      server:(value.servers || [])[0] || ({id:"local",name:"Wisp server"}),
      hangouts:value.hangouts || [], conversations:value.conversations || [],
      messages:value.messages || [], room_invitations:value.room_invitations || []
    }]
    var conversations=[],messages=[],invitations=[]
    states.forEach(function(state) {
      ChatLogic.visibleConversations(state.conversations || [],state.hangouts || []).forEach(function(c) { conversations.push(root.scopedConversation(state.server,c)) })
      var stateMessages=state.messages || [],stateInvitations=state.room_invitations || []
      stateMessages.forEach(function(m) { messages.push(root.scopedMessage(state.server,m)) })
      stateInvitations.forEach(function(i) { invitations.push(Object.assign({},i,{server_id:String(state.server.id)})) })
    })
    return {conversations:conversations,messages:messages,room_invitations:invitations}
  }
  function roomEventsForSnapshots(previous,next,eventName) {
    if (!(next.server_states || []).length) return ChatLogic.roomSoundEvents(previous,next,eventName)
    var result=[],previousStates=previous ? previous.server_states || [] : [],nextStates=next.server_states || []
    nextStates.forEach(function(state) {
      var old=previousStates.filter(function(candidate) { return String(candidate.server.id)===String(state.server.id) })[0] || null
      result=result.concat(ChatLogic.roomSoundEvents(old,state,eventName))
    })
    return result
  }
  function scopeForConversation(id) {
    var conversation = conversationById(id)
    if (conversation) return {server_id:String(conversation.server_id),conversation_id:String(conversation.raw_id)}
    var text = String(id || ""), split = text.indexOf("::")
    if (split > 0) return {server_id:text.slice(0, split),conversation_id:text.slice(split + 2)}
    return {server_id:String(activeServer.id || ""),conversation_id:text}
  }
  function messageById(id) {
    for (var i=0;i<messages.length;i++) if (String(messages[i].id)===String(id)) return messages[i]
    return null
  }
  function scopeForMessage(id) {
    var message=messageById(id)
    return {server_id:String(message ? message.server_id : activeServer.id || ""),message_id:String(id)}
  }
  function withConversationScope(id, values) {
    return Object.assign({}, values || {}, scopeForConversation(id))
  }
  readonly property var serverStates: {
    if ((snapshot.server_states || []).length) return snapshot.server_states
    var fallback = (snapshot.servers || [])[0] || ({id:"local",name:"Wisp server",connected:daemonConnected})
    return [{server:fallback,self:snapshot.self,friends:snapshot.friends || [],hangouts:snapshot.hangouts || [],knocks:snapshot.knocks || [],room_invitations:snapshot.room_invitations || [],conversations:snapshot.conversations || [],messages:snapshot.messages || [],spots:snapshot.spots || [],devices:snapshot.devices || []}]
  }
  readonly property var servers: {
    var values = (snapshot.servers || []).slice()
    if (!values.length) values = serverStates.map(function(state) { return state.server })
    return values
  }
  property string activeServerId: ""
  function selectServer(id) {
    id = String(id || "")
    if (!servers.some(function(server) { return String(server.id) === id })) return
    activeServerId = id
    workspaceLayout.selectedServerId = id
    serverSettings = ({name:"",role:"",members:[],categories:[],channels:[],rooms:[]})
    serverSettingsFeedback = ""
    if (canManageServer) refreshServerSettings()
  }
  onServersChanged: {
    if (!servers.length) return
    var preferred = String(workspaceLayout.selectedServerId || snapshot.selected_server_id || activeServerId || "")
    if (!servers.some(function(server) { return String(server.id) === preferred })) preferred = String(servers[0].id)
    if (activeServerId !== preferred) activeServerId = preferred
  }
  readonly property var activeServer: servers.filter(function(server) { return String(server.id) === root.activeServerId })[0] || servers[0] || ({id:"",name:"Wisp server",connected:false})
  readonly property var activeServerState: serverStates.filter(function(state) { return String(state.server.id) === String(root.activeServer.id) })[0]
    || ({server:activeServer,self:{display_name:configuredProfile,presence:"away",connection:"connecting_to_server",server_owner:false,server_admin:false},friends:[],hangouts:[],knocks:[],room_invitations:[],conversations:[],messages:[],spots:[],devices:[]})
  readonly property string voiceServerId: String(snapshot.voice_server_id || snapshot.selected_server_id || (servers[0] || {}).id || "")
  readonly property var voiceServerState: serverStates.filter(function(state) { return String(state.server.id)===root.voiceServerId })[0] || serverStates[0] || ({})
  readonly property var voiceHangouts: (voiceServerState.hangouts || []).map(function(room) { return Object.assign({},room,{server_id:root.voiceServerId,server_name:String((voiceServerState.server || {}).name || "")}) })
  readonly property var voiceFriends: voiceServerState.friends || []
  readonly property var voiceSpots: voiceServerState.spots || []
  readonly property var selfState: {
    var selected = activeServerState.self || snapshot.self || ({})
    var mediaOwner = snapshot.self || selected
    return Object.assign({}, selected, {
      muted:!!mediaOwner.muted,
      deafened:!!mediaOwner.deafened,
      sharing:!!mediaOwner.sharing,
      hangout_id:mediaOwner.hangout_id,
      push_to_talk:mediaOwner.push_to_talk,
      media:mediaOwner.media
    })
  }
  readonly property var friends: (activeServerState.friends || []).map(function(friend) { return Object.assign({},friend,{server_id:String(root.activeServer.id),server_name:String(root.activeServer.name)}) })
  readonly property var sortedFriends: FriendLogic.sorted(friends, friendPreferences.favorites)
  readonly property var hangouts: (activeServerState.hangouts || []).map(function(room) { return Object.assign({},room,{server_id:String(root.activeServer.id),server_name:String(root.activeServer.name)}) })
  readonly property var knocks: (activeServerState.knocks || []).map(function(knock) { return Object.assign({},knock,{server_id:String(root.activeServer.id)}) })
  property double invitationClock: Date.now()
  property var invitationRequests: ({})
  property string invitationFeedback: ""
  readonly property var roomInvitations: (activeServerState.room_invitations || []).filter(function(i) { return Date.parse(i.expires_at) > root.invitationClock }).map(function(invite) { return Object.assign({},invite,{server_id:String(root.activeServer.id)}) })
  readonly property var currentVoiceRoom: voiceHangouts.filter(function(h) { return h.id === root.selfState.hangout_id })[0] || null
  Timer { interval: 1000; repeat: true; running: true; onTriggered: root.invitationClock = Date.now() }
  Timer { id: invitationFeedbackTimer; interval: 3000; onTriggered: root.invitationFeedback = "" }

  function inviteToRoom(friend) {
    if (!currentVoiceRoom || invitationRequests[friend.id]) return
    var id = send("send_voice_invite", {server_id:String(currentVoiceRoom.server_id || activeServer.id),hangout_id:currentVoiceRoom.id, user_id:friend.id})
    if (id) {
      invitationRequests = replaceEntry(invitationRequests, friend.id, true)
      requests[id] = {kind:"roomInvite", key:friend.id, name:friend.display_name, serverId:String(currentVoiceRoom.server_id || activeServer.id)}
    }
  }
  function needsEncryptedRoomAccess(friend) {
    if (!currentVoiceRoom || !(privacyStatus.configured || snapshot.chat_encryption_required)) return false
    var spot = voiceSpots.filter(function(s) { return s.active_hangout_id === root.currentVoiceRoom.id })[0]
    var conversation = conversationById(spot ? "spot:" + spot.id : "hangout:" + currentVoiceRoom.id)
    return !conversation || !(conversation.members || []).some(function(p) { return p.id === friend.id })
  }
  function respondRoomInvitation(invitation, accept) {
    var key = String(invitation.invitation_id || invitation.id)
    if (invitationRequests[key]) return
    var id = send("respond_room_invitation", {server_id:String(invitation.server_id || activeServer.id),id:key, accept:accept})
    if (id) {
      invitationRequests = replaceEntry(invitationRequests, key, true)
      requests[id] = {kind:"roomInviteResponse", key:key}
    }
  }
  function openRoomChat(room, persistent) {
    var spot = persistent ? room : spots.filter(function(s) { return s.active_hangout_id === room.id })[0]
    var id = scopedConversationId(String(room.server_id || activeServer.id),spot ? "spot:" + spot.id : "hangout:" + room.id)
    if (conversationById(id)) selectConversation(id)
    else lastError = "Join this room or ask an administrator for access to its chat."
  }
  readonly property var conversations: {
    var result=[]
    serverStates.forEach(function(state) {
      var visible=ChatLogic.visibleConversations(state.conversations || [],state.hangouts || [])
      visible.forEach(function(conversation) { result.push(root.scopedConversation(state.server,conversation)) })
    })
    pendingCreatedConversations.forEach(function(conversation) {
      if (!result.some(function(current) { return String(current.id)===String(conversation.id) })) result.push(conversation)
    })
    return result
  }
  readonly property var messages: {
    var result=[]
    serverStates.forEach(function(state) { (state.messages || []).forEach(function(message) { result.push(root.scopedMessage(state.server,message)) }) })
    return result
  }
  readonly property var spots: (activeServerState.spots || []).map(function(spot) { return Object.assign({},spot,{server_id:String(root.activeServer.id),server_name:String(root.activeServer.name),conversation_id:root.scopedConversationId(root.activeServer.id,"spot:"+spot.id)}) })
  readonly property int roomCount: {
    var activeFromSpots = ({})
    spots.forEach(function(spot) {
      if (spot.active_hangout_id) activeFromSpots[String(spot.active_hangout_id)] = true
    })
    return spots.length + hangouts.filter(function(room) {
      return !activeFromSpots[String(room.id)]
    }).length
  }
  readonly property var devices: activeServerState.devices || []
  readonly property var lastInvite: snapshot.last_invite || null
  property var lastAccountInvite: null
  property string activeConversationId: ""
  property string lastConversationId: ""
  property string previousActiveId: ""
  onActiveConversationIdChanged: {
    if (previousActiveId && previousActiveId !== activeConversationId) lastConversationId = previousActiveId
    previousActiveId = activeConversationId
  }
  readonly property var lastConversation: lastConversationId !== activeConversationId ? conversationById(lastConversationId) : null
  readonly property var unreadConversations: conversations.filter(function(c) {
    return Number(c.unread_count || 0) > 0 && String(c.id) !== activeConversationId
  })
  property string pendingDirectName: ""
  property string pendingDirectServerId: ""
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
  readonly property var rawSpeakers: {
    var names = (mediaState.active_speakers || []).slice(), levels = mediaState.remote_audio_levels || ({})
    Object.keys(levels).forEach(function(name) {
      if (Number(levels[name]) >= 25 && (root.mediaState.remote_audio_participants || []).indexOf(name) >= 0 && names.indexOf(name) < 0) names.push(name)
    })
    var localAudio = mediaState.audio || ({}), localSpeaking = Number(localAudio.input_level || 0) >= 25
    if (localSpeaking && !effectiveMuted && names.indexOf(selfState.display_name) < 0) names.push(selfState.display_name)
    return names
  }
  property var speakerReleaseTimes: ({})
  property double speakerClock: Date.now()
  // A short release hold bridges VAD gaps between syllables. Muting/leaving
  // always wins, and steady activity never expires on an arbitrary timeout.
  readonly property var activeSpeakers: {
    if (!selfState.hangout_id) return []
    var names = rawSpeakers.slice()
    Object.keys(speakerReleaseTimes).forEach(function(name) {
      if (speakerReleaseTimes[name] > speakerClock && names.indexOf(name) < 0) names.push(name)
    })
    return names.filter(function(name) {
      return name === root.selfState.display_name ? !root.effectiveMuted : root.remoteMutedParticipants.indexOf(name) < 0
    })
  }
  property var previousSpeakers: []
  onRawSpeakersChanged: {
    var now = Date.now(), next = Object.assign({}, speakerReleaseTimes)
    previousSpeakers.forEach(function(name) { if (root.rawSpeakers.indexOf(name) < 0) next[name] = now + 700 })
    rawSpeakers.forEach(function(name) { delete next[name] })
    previousSpeakers = rawSpeakers.slice(); speakerReleaseTimes = next; speakerClock = now
  }
  Timer {
    interval: 100; repeat: true; running: Object.keys(root.speakerReleaseTimes).length > 0
    onTriggered: {
      var now = Date.now(), next = ({})
      Object.keys(root.speakerReleaseTimes).forEach(function(name) {
        if (root.selfState.hangout_id && root.speakerReleaseTimes[name] > now) next[name] = root.speakerReleaseTimes[name]
      })
      root.speakerClock = now; root.speakerReleaseTimes = next
    }
  }
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
    "processing_latency_ms": 30,
    "deepfilter_strength": 100,
    "processing_time_us": 0,
    "processing_deadline_misses": 0,
    "capture_queue_ms": 0
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
    var match = localConfig.text().match(/(?:^|\n)WISP_PROFILE=([^\r\n]+)(?:\n|$)/)
    return match ? String(match[1]) : ""
  }

  function localPreviewUrl(fileStem, revision) {
    if (!runtimeDir || revision < 1) return ""
    return "file://" + runtimeDir + "/wisp/" + fileStem + "-"
      + String(revision % 2) + ".bmp"
  }

  FileView {
    id: localConfig
    path: root.configHome + "/wisp/account.env"
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
      property string policy: "other_chats"
      property var mutedChats: []
      property bool roomSounds: true
      property bool selfRoomSounds: true
      property var eventSounds: ({})
    }
  }

  Process {
    id: notificationPlayer
    onExited: function(exitCode, exitStatus) {
      if (exitCode !== 0) root.notificationError = "Could not play this sound. Choose a readable audio file and check that pw-play is installed."
      if (root.soundQueue.length) {
        var next=root.soundQueue[0]; root.soundQueue=root.soundQueue.slice(1)
        Qt.callLater(function() { root.soundPlaybackBusy = false; root.playNotificationSound(next) })
      } else root.soundPlaybackBusy = false
    }
  }

  function playNotificationSound(kind) {
    kind = kind || "message"
    if (notificationMuted || notificationVolume <= 0) return
    if (kind.indexOf("self_") === 0 && !selfRoomNotificationSounds) return
    if (kind.indexOf("member_") === 0 && !roomNotificationSounds) return
    if (soundPlaybackBusy) {
      if (soundQueue.length < 6 && soundQueue.indexOf(kind) < 0) soundQueue = soundQueue.concat([kind])
      return
    }
    notificationError = ""
    // Process arguments need a real path, not Quickshell's virtual qs: URL.
    var soundDirectory = Quickshell.env("WISP_SOUND_DIR") || configHome + "/quickshell/wisp/assets"
    var path = String((kind === "message" ? notificationSoundPath : eventSoundPaths[kind]) || soundDirectory + "/" + kind + ".wav")
    if (path.indexOf("file://") === 0) path = decodeURIComponent(path.slice(7))
    if (path.charAt(0) !== "/") {
      notificationError = "Choose an absolute path to a local audio file."
      return
    }
    notificationPlayer.command = ["pw-play", "--volume",
      String(Math.max(0, Math.min(100, notificationVolume)) / 100), path]
    soundPlaybackBusy = true
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
    if (roomInvitations.length) return "Voice invite · " + roomInvitations[0].from.display_name
    if (knocks.length > 0) return "󰍬  " + String(knocks[0].from.display_name || "Friend") + " knocked"
    if (unreadMessages > 0 && !inHangout) return "󰍩  " + String(unreadMessages) + " unread"
    var names = []
    if (inHangout) {
      for (var h = 0; h < voiceHangouts.length; h++) {
        if (voiceHangouts[h].id !== selfState.hangout_id) continue
        var members = voiceHangouts[h].members || []
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
      for (var h = 0; h < voiceHangouts.length; h++) {
        if (voiceHangouts[h].id !== selfState.hangout_id) continue
        var members = voiceHangouts[h].members || []
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
    var previousFlat=receivedSnapshot ? flattenedSnapshot(snapshot) : null
    var nextFlat=flattenedSnapshot(next)
    var incoming = ChatLogic.incomingConversationIds(previousFlat, nextFlat, eventName)
    var roomEvents = roomEventsForSnapshots(receivedSnapshot ? snapshot : null,next,eventName)
    var knownInvites = (previousFlat ? previousFlat.room_invitations : []).map(function(i) { return String(i.server_id)+":"+String(i.id) })
    var newInvite = receivedSnapshot && nextFlat.room_invitations.some(function(i) {
      return Date.parse(i.expires_at) > Date.now() && knownInvites.indexOf(String(i.server_id)+":"+String(i.id)) < 0
    })
    snapshot = next
    pendingCreatedConversations = pendingCreatedConversations.filter(function(pending) {
      return !nextFlat.conversations.some(function(current) { return String(current.id)===String(pending.id) })
    })
    if (activeConversationId) {
      var nextVisibleConversations = conversations
      var activeStillVisible = nextVisibleConversations.some(function(conversation) {
        return String(conversation.id) === String(activeConversationId)
      })
      if (!activeStillVisible) activeConversationId = ""
    }
    receivedSnapshot = true
    if (notificationSoundsEnabled && newInvite) playNotificationSound("room_invite")
    if (notificationSoundsEnabled) roomEvents.forEach(function(kind) { root.playNotificationSound(kind) })
    if (notificationSoundsEnabled && incoming.some(function(id) {
      return ChatLogic.shouldNotifyChat(id, root.focusedConversationId, root.appFocused,
        root.notificationPolicy, root.mutedNotificationChats, root.notificationMuted, root.notificationVolume)
    })) playNotificationSound()
    var nextSelf = next["self"] || ({})
    var nextMedia = nextSelf.media || ({})
    var nextShare = nextMedia.screen_share || ({})
    var nextCamera = nextMedia.camera || ({})
    lastError = String(nextMedia.error || nextMedia.surface_error
      || nextShare.error || nextCamera.error || "")
    if (pendingDirectName !== "") {
      var wanted = pendingDirectName
      var wantedServer = pendingDirectServerId
      var nextConversations = conversations
      for (var i = 0; i < nextConversations.length; i++) {
        if (String(nextConversations[i].kind) === "direct"
            && String(nextConversations[i].label) === wanted
            && (!wantedServer || String(nextConversations[i].server_id) === wantedServer)) {
          activeConversationId = String(nextConversations[i].id)
          pendingDirectName = ""
          pendingDirectServerId = ""
          send("mark_conversation_read", withConversationScope(activeConversationId))
          break
        }
      }
    }
    markVisibleConversationRead()
  }

  function markVisibleConversationRead() {
    var c = conversationById(focusedConversationId)
    if (!c || !c.last_message || !c.unread_count) return
    var id = String(c.last_message.id)
    if (lastReadMessageId === id) return
    lastReadMessageId = id
    send("mark_conversation_read", withConversationScope(c.id))
  }

  function conversationById(id) {
    for (var i = 0; i < conversations.length; i++)
      if (String(conversations[i].id) === String(id)) return conversations[i]
    // Older local layouts stored unscoped IDs. Resolve them against the active
    // server first so upgrading does not discard a user's open workspace.
    for (var j = 0; j < conversations.length; j++)
      if (String(conversations[j].server_id) === String(activeServer.id)
          && String(conversations[j].raw_id) === String(id)) return conversations[j]
    return null
  }

  function messagesFor(id) {
    var result = []
    if (!id) return result
    var conversation=conversationById(id)
    var canonical=conversation ? String(conversation.id) : String(id)
    for (var i = 0; i < messages.length; i++)
      if (String(messages[i].conversation_id) === canonical) result.push(messages[i])
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
  function conversationKeys(id) {
    var value=String(id || ""), conversation=conversationById(value)
    var canonical=conversation ? String(conversation.id) : value
    var raw=conversation ? String(conversation.raw_id) : value
    return canonical===raw ? [canonical] : [canonical,raw]
  }
  function conversationValue(map,id,fallback) {
    var keys=conversationKeys(id)
    for(var i=0;i<keys.length;i++) if(map[keys[i]]!==undefined)return map[keys[i]]
    return fallback
  }
  function replaceConversationEntry(map,id,value) {
    var next=Object.assign({},map),keys=conversationKeys(id)
    keys.forEach(function(key){if(value===undefined)delete next[key];else next[key]=value})
    return next
  }
  function draftFor(id) { return String(conversationValue(drafts,id,"") || "") }
  function setDraft(id, value) {
    if (id && draftFor(id) !== value) drafts = replaceConversationEntry(drafts, id, value)
  }
  function pasteClipboard(conversationId) {
    if (!conversationId || conversationValue(sendingConversations,conversationId,false)) return
    var id = send("paste_clipboard", {})
    if (id) {
      requests[id] = { kind: "paste", conversationId: conversationId }
      importingConversations = replaceConversationEntry(importingConversations, conversationId, Number(conversationValue(importingConversations,conversationId,0)) + 1)
    }
  }
  function attachmentsFor(id) { return conversationValue(pendingAttachments,id,[]) || [] }
  function setAttachmentKeep(conversationId, token, keep) {
    pendingAttachments = replaceConversationEntry(pendingAttachments, conversationId, attachmentsFor(conversationId).map(function(a) {
      return a.token === token ? Object.assign({}, a, {keep:keep}) : a
    }))
  }
  function transferLabel(direction, id) {
    var value = transferProgress[direction + ":" + id]
    return value && value.total > 0 ? " " + Math.min(100, Math.floor(value.bytes * 100 / value.total)) + "%" : "…"
  }
  function importChatFiles(conversationId, urls) {
    if (!conversationId || conversationValue(sendingConversations,conversationId,false)) return
    var values = []
    for (var i = 0; i < urls.length; i++) values.push(String(urls[i]))
    var id = send("import_chat_files", {urls: values})
    if (id) {
      requests[id] = {kind: "import", conversationId: conversationId}
      importingConversations = replaceConversationEntry(importingConversations, conversationId, Number(conversationValue(importingConversations,conversationId,0)) + 1)
    }
  }
  function removeAttachment(conversationId, token, alreadySent) {
    if (!alreadySent) send("discard_attachment_draft", {token: token})
    pendingAttachments = replaceConversationEntry(pendingAttachments, conversationId,
      attachmentsFor(conversationId).filter(function(a) { return a.token !== token }))
  }
  function sendAttachmentQueue(conversationId, tokens, caption, originalText) {
    var attachment = attachmentsFor(conversationId).filter(function(a) { return a.token === tokens[0] })[0]
    sendingConversations = replaceConversationEntry(sendingConversations, conversationId, true)
    var id = send("send_attachment_message", withConversationScope(conversationId, {token: tokens[0], caption: caption, keep:!!(attachment && attachment.keep)}))
    if (id) {
      requests[id] = {kind: "send", conversationId: conversationId, text: originalText, token: tokens[0], remaining: tokens.slice(1)}
    } else sendingConversations = replaceConversationEntry(sendingConversations, conversationId, undefined)
  }
  function sendComposedMessage(conversationId) {
    if (!conversationId || conversationValue(sendingConversations,conversationId,false) || conversationValue(importingConversations,conversationId,0)) return
    var text = draftFor(conversationId).trim()
    var attachments = attachmentsFor(conversationId)
    if (attachments.length > 0) {
      sendAttachmentQueue(conversationId, attachments.map(function(a) { return a.token }), text, draftFor(conversationId))
      return
    }
    if (!text) return
    sendingConversations = replaceConversationEntry(sendingConversations, conversationId, true)
    var id = send("send_message", withConversationScope(conversationId, {text: text}))
    if (id) {
      requests[id] = {kind: "send", conversationId: conversationId, text: draftFor(conversationId), token: ""}
    } else sendingConversations = replaceConversationEntry(sendingConversations, conversationId, undefined)
  }
  function saveChatFile(messageId) {
    if (savingFiles[messageId]) return
    var id = send("save_chat_file", scopeForMessage(messageId))
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
    var id = send("load_chat_image", scopeForMessage(messageId))
    if (id) {
      imageRequests[messageId] = true
      requests[id] = {kind: "image", messageId: messageId}
    }
  }
  function copyChatImage(messageId) {
    var id=send("copy_chat_image",scopeForMessage(messageId))
    if(id)requests[id]={kind:"copyImage"}
    return id || ""
  }
  function copyChatText(text) { return send("copy_chat_text", {text: String(text)}) }
  function finishRequest(message) {
    var action = requests[message.id]
    if (!action) return
    delete requests[message.id]
    var value = message.value || ({})
    var conversationId = action.conversationId
    if (action.kind === "privacyStatus" || action.kind === "privacySetup" || action.kind === "privacyExport") {
      if (message.ok) {
        if (action.kind !== "privacyExport") privacyStatus = value
        if (action.kind === "privacySetup") { privacyFeedback = "Encryption configured; recovery file saved locally."; settingsSaved() }
        if (action.kind === "privacyExport") privacyFeedback = "Recovery file saved locally. Keep it private."
      } else privacyFeedback = message.error ? String(message.error.message) : "Privacy operation failed"
      privacyBusy = false
    } else if (action.kind === "serverSettings") {
      if (message.ok) serverSettings = value
      else serverSettingsFeedback = message.error ? String(message.error.message || "Could not load server settings") : "Could not load server settings"
      serverSettingsBusy = false
    } else if (action.kind === "serverMutation") {
      serverSettingsBusy = false
      if (message.ok) {
        serverSettingsFeedback = "Changes saved"
        settingsSaved()
        refreshServerSettings()
      } else {
        serverSettingsFeedback = message.error ? String(message.error.message || "Could not update server") : "Could not update server"
      }
    } else if (action.kind === "accountInvite") {
      if (message.ok) lastAccountInvite = value
      else lastError = message.error ? String(message.error.message || "Could not create invite") : "Could not create invite"
    } else if (action.kind === "roomInvite" || action.kind === "roomInviteResponse") {
      invitationRequests = replaceEntry(invitationRequests, action.key, undefined)
      if (message.ok && action.kind === "roomInvite") {
        invitationFeedback = value.already_pending ? "Invitation already pending" : "Voice invite sent to " + action.name
        invitationFeedbackTimer.restart()
        if (value.conversation_id) selectConversation(scopedConversationId(action.serverId,value.conversation_id))
      }
    } else if (action.kind === "copyImage") {
      imageCopyFinished(String(message.id),!!message.ok,message.error ? String(message.error.message || "Could not copy image") : "")
    } else if (action.kind === "watchVideo") {
      if (message.ok && action.open) {
        watchedMedia = replaceEntry(watchedMedia, action.key, action.video)
        mediaWatchReady(action.video)
      } else if (!action.open || !message.ok) watchedMedia = replaceEntry(watchedMedia, action.key, undefined)
    } else if (action.kind === "setting") {
      if (message.ok) settingsSaved()
      else settingsSaveFailed()
    } else if (action.kind === "edit" || action.kind === "delete") {
      messageMutationFinished(action.messageId, action.kind, !!message.ok,
        message.error ? String(message.error.message || "Could not update message") : "")
    } else if (action.kind === "clear") {
      historyClearFinished(action.conversationId, !!message.ok, message.error ? String(message.error.message || "Could not clear history") : "")
    } else if (action.kind === "newChat") {
      var created = !!message.ok && !!value.id
      var createdId = created ? scopedConversationId(action.serverId, value.id) : ""
      if (created) {
        var server=servers.filter(function(candidate) { return String(candidate.id)===String(action.serverId) })[0] || ({id:action.serverId,name:"Wisp server"})
        pendingCreatedConversations = pendingCreatedConversations.concat([scopedConversation(server,value)])
      }
      chatCreationFinished(String(message.id), created, createdId,
        created ? "" : message.error ? String(message.error.message || "Could not create chat") : "Could not create chat")
    } else if (action.kind === "room") {
      if (message.ok && action.action === "create_room" && value.id) activeConversationId = scopedConversationId(action.serverId,value.id)
      roomActionFinished(action.action, !!message.ok, message.error ? String(message.error.message || "Could not update room") : "")
    } else if (action.kind === "send") {
      if (message.ok) {
        if (action.text !== undefined && draftFor(conversationId) === action.text) setDraft(conversationId, "")
        if (action.token) removeAttachment(conversationId, action.token, true)
        if (action.remaining && action.remaining.length > 0) {
          sendAttachmentQueue(conversationId, action.remaining, "", undefined)
          return
        }
      }
      // Clear acknowledged content before re-enabling Enter and Send.
      sendingConversations = replaceConversationEntry(sendingConversations, conversationId, undefined)
    } else if (action.kind === "paste" || action.kind === "import") {
      importingConversations = replaceConversationEntry(importingConversations, conversationId,
        Math.max(0, Number(conversationValue(importingConversations,conversationId,1)) - 1))
      if (message.ok) {
        var added = value.attachments || (value.token ? [value] : [])
        if (added.length > 0) pendingAttachments = replaceConversationEntry(pendingAttachments, conversationId, attachmentsFor(conversationId).concat(added))
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

  function setPresence(value) { send("set_presence", {server_id:String(activeServer.id || ""), "presence": value }) }
  function saveSetting(name, args) {
    var id = send(name, args)
    if (id) requests[id] = {kind: "setting"}
    else settingsSaveFailed()
    return id
  }
  function editChatMessage(messageId, text) {
    var id = send("edit_message", Object.assign(scopeForMessage(messageId), {text: text}))
    if (id) requests[id] = {kind: "edit", messageId: messageId}
    return !!id
  }
  function deleteChatMessage(messageId) {
    var id = send("delete_message", scopeForMessage(messageId))
    if (id) requests[id] = {kind: "delete", messageId: messageId}
  }
  function joinFriend(name) { send("join_friend", {server_id:String(activeServer.id || ""), "friend": name }) }
  function joinHangout(id) { send("join_hangout", {server_id:String(activeServer.id || ""), "hangout_id": id }) }
  function joinSpot(id) { send("join_spot", {server_id:String(activeServer.id || ""), "spot_id": id }) }
  function openDirect(friendName) {
    pendingDirectName = String(friendName)
    pendingDirectServerId = String(activeServer.id || "")
    send("open_direct", {server_id:pendingDirectServerId, "friend": String(friendName) })
  }
  function createChat(group, args) {
    var serverId=String(args.server_id || activeServer.id || "")
    var values=Object.assign({},args,{server_id:serverId})
    var request = send(group ? "create_group" : "open_direct", values)
    if (request) requests[request] = {kind:"newChat",serverId:serverId}
    return request || ""
  }
  function selectConversation(id) {
    var c = conversationById(id)
    activeConversationId = c ? String(c.id) : String(id)
    if (c && c.tab_closed) send("set_conversation_tab", withConversationScope(id, {closed:false}))
    send("mark_conversation_read", withConversationScope(activeConversationId))
  }
  function exitConversation(id) {
    send("set_conversation_tab", withConversationScope(id, {closed:true}))
  }
  function clearChatHistory(id, forEveryone) {
    var request = send("clear_chat_history", withConversationScope(id, {for_everyone:!!forEveryone}))
    if (request) requests[request] = {kind:"clear",conversationId:String(id)}
    return !!request
  }
  function setFileRetention(id, keep) {
    send("set_file_retention", Object.assign(scopeForMessage(id),{keep:keep}))
  }
  function roomAction(action, args) {
    var values=Object.assign({},args || {})
    if (values.conversation_id) values=withConversationScope(values.conversation_id,values)
    else values.server_id=String(activeServer.id || "")
    var request = send(action, values)
    if (request) requests[request] = {kind:"room",action:action,serverId:String(values.server_id || activeServer.id || "")}
    return !!request
  }
  function closeConversation() { activeConversationId = "" }
  function sendMessage(text) {
    var trimmed = String(text || "").trim()
    if (!activeConversationId || !trimmed) return false
    send("send_message", withConversationScope(activeConversationId,{text:trimmed}))
    return true
  }
  function createInvite(profile, expiresMinutes) {
    send("create_invite", {
      "profile": String(profile),
      "expires_in_minutes": Number(expiresMinutes || 30)
    })
  }
  function createAccountInvite(kind, conversationId, expiresMinutes) {
    var scope = conversationId ? scopeForConversation(conversationId) : {server_id:String(activeServer.id || ""),conversation_id:null}
    var id = send("create_account_invite", {
      "server_id": scope.server_id,
      "kind": String(kind || "friend"),
      "conversation_id": scope.conversation_id,
      "expires_in_minutes": Number(expiresMinutes || 30)
    })
    if (id) requests[id] = {kind:"accountInvite"}
    return id || ""
  }
  function refreshDevices() { send("list_devices", {server_id:String(activeServer.id || "")}) }
  function revokeDevice(id) { send("revoke_device", {server_id:String(activeServer.id || ""), "device_id": String(id) }) }
  function respondKnock(id, response) { send("respond_knock", {server_id:String(activeServer.id || ""), "knock_id": id, "response": response }) }
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
  function setDeepfilterStrength(strength) {
    saveSetting("set_deepfilter_strength", { "strength": Math.max(0, Math.min(100, Math.round(strength))) })
  }
  function refreshVideoDevices() { send("refresh_video_devices", {}) }
  function setCameraDevice(id) { saveSetting("set_camera_device", { "id": id }) }
  function setVideoQuality(quality) { saveSetting("set_video_quality", { "quality": quality }) }
  function setVideoCodec(codec) { saveSetting("set_video_codec", { "codec": codec }) }
  function openSurface() { send("open_surface", {}) }
  function closeSurface() { send("close_surface", {}) }
  function toggleSurface() { mediaState.surface_open ? closeSurface() : openSurface() }
  function watchVideo(video, open) {
    if (!video) return
    if (delegateMediaToDesktop) { desktopWatchRequested(String(video.participant),String(video.source),open); return }
    var key = JSON.stringify([String(video.participant), String(video.source)])
    var id = send("watch_video", {
      "participant": String(video.participant || ""),
      "source": String(video.source || "screen_share"),
      "hosted": !!mediaTileHost,
      "open": open
    })
    if (id && mediaTileHost) requests[id] = {kind: "watchVideo", key: key, video: video, open: open}
    if (!open) watchedMedia = replaceEntry(watchedMedia, key, undefined)
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
          root.invitationRequests = ({})
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
