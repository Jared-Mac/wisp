// Index labels and aliases only; never search account fields or private content.
var entries = [
  {section:"appearance",target:"chatLayoutSetting",label:"Chat layout",keywords:"grouped compact soft groups message density avatars inline sender headers"},
  {section:"notifications",target:"autoAwaySetting",label:"Automatic Away",keywords:"presence idle inactive pc activity computer desktop mobile phone notifications thirty minutes"},
  {section:"notifications",target:"idleThresholdSetting",label:"PC inactivity time",keywords:"away phone mobile alerts gaming controller keyboard mouse threshold minutes"},
  {section:"updates",target:"checkForUpdates",label:"Check for updates",keywords:"release version download manual update"},
  {section:"updates",target:"update-automatic",label:"Automatic updates",keywords:"install disable enable update"},
  {section:"updates",target:"update-check_on_launch",label:"Check for updates at launch",keywords:"startup restart update"},
  {section:"updates",target:"update-background_checks",label:"Background update checks",keywords:"disable enable running periodic"},
  {section:"updates",target:"updateInterval",label:"Update check interval",keywords:"minutes frequency customize automatic"},
  {section:"soundboard",target:"settingsSoundboard",label:"Server soundboard",keywords:"audio sounds upload clips effects preview play voice starter default"},
  {section:"media",target:"audioTestSection",label:"Microphone test",keywords:"audio hear yourself record sample original processed voice playback"},
  {section:"media",target:"audioDiagnosticsSection",label:"Processing status",keywords:"audio noise cleanup quality performance latency diagnostics"},
  {section:"profile",target:"accountEmojiLibrary",label:"My custom emojis",keywords:"account emoji library upload reactions"},
  {section:"server",target:"serverEmojiLibrary",label:"Server custom emojis",keywords:"admin emoji library upload reactions"},
  {section:"media", target:"voiceReconnectSetting", label:"Automatically reconnect voice", keywords:"audio connection disconnect server restart network retry recovery"},
  {section:"profile", target:"profilePicture", label:"Profile picture", keywords:"avatar image photo upload remove"},
  {section:"appearance", target:"showAvatarsSetting", label:"Show user avatars", keywords:"hide pictures icons simplify lists"},
  {section:"profile", target:"profileDisplayName", label:"Display name", keywords:"account profile username nickname"},
  {section:"profile", target:"profileCurrentPassword", label:"Change password", keywords:"account profile login security"},
  {section:"privacy", target:"privacyDetailsSection", label:"Account sync", keywords:"backup restore sync device secure sign in password recovery encryption keys"},
  {section:"profile", target:"profileRecoveryEmail", label:"Recovery email", keywords:"account forgot password reset verify verification email"},
  {section:"profile", target:"profileTwoFactorStatus", label:"Two-factor authentication (planned)", keywords:"account profile security 2fa mfa"},
  {section:"video", target:"streamsAsTilesSetting", label:"Open streams in windows or tiles", keywords:"audio video screen share watch popout pop-out default dock anchor"},
  {section:"video", target:"settingsCamera", label:"Camera", keywords:"audio video webcam device input"},
  {section:"video", target:"settingsVideoQuality", label:"Publishing quality", keywords:"audio video stream screen share resolution 720p 1080p 1440p fps"},
  {section:"video", target:"settingsVideoCodec", label:"Video codec", keywords:"audio stream encoding h264 vp8 av1 hardware"},
  {section:"media", target:"settingsMicrophone", label:"Microphone", keywords:"audio input device volume level mic"},
  {section:"media", target:"settingsSpeaker", label:"Speaker", keywords:"audio output device headphones sound"},
  {section:"media", target:"settingsProcessing", label:"Audio processing and echo cancellation", keywords:"microphone noise suppression denoiser filter natural clear studio deepfilter deepnet aec echo cancellation"},
  {section:"media", target:"settingsPushToTalk", label:"Push to talk", keywords:"audio microphone ptt mute"},
  {section:"media", target:"settingsShortcut", label:"Push-to-talk shortcut", keywords:"audio ptt global keyboard hotkey keybind"},
  {section:"appearance", target:"settingsAppearance", label:"Interface style", keywords:"appearance theme soft graphite daylight hearth clean tui performative herdr terminal classic"},
  {section:"appearance", target:"settingsPalette", label:"Color palette", keywords:"appearance theme colors graphite daylight hearth solarized japan ash olive"},
  {section:"appearance", target:"settingsAccents", label:"Color accents", keywords:"appearance colors names chats highlights"},
  {section:"appearance", target:"settingsDesktopPosition", label:"Desktop position", keywords:"tray panel screen corner display anchor", panel:true},
  {section:"notifications", target:"incomingDmsAsTilesSetting", label:"Open incoming DMs in new tiles", keywords:"inbox unread messages automatic pending chat navigation"},
  {section:"notifications", target:"channelsAsTilesSetting", label:"Open channels and rooms in new tiles", keywords:"chat navigation default reuse pane window"},
  {section:"notifications", target:"settingsNotifications", label:"Message notification sounds", keywords:"chat mute sound alerts focus background policy"},
  {section:"notifications",target:"friendRoomNotificationsSetting",label:"Friends joining rooms · desktop alerts",keywords:"pc notification voice activity friends join rooms popup banner"},
  {section:"notifications",target:"friendRoomTimingSetting",label:"Room activity timing and home servers",keywords:"desktop notification cooldown home servers multiple first joined sound empty background"},
  {section:"notifications", target:"mentionsOnlySetting", label:"Only messages that @mention me", keywords:"mentions names ping notification filter sounds"},
  {section:"notifications", target:"settingsNotificationVolume", label:"Notification volume and custom sound", keywords:"chat alert chime test file"},
  {section:"notifications", target:"audioControlSoundsSetting",label:"Mute and deafen sounds",keywords:"mic microphone unmute undeafen audio cues"},
  {section:"notifications", target:"screenShareSoundsSetting",label:"Screen-share start and stop sounds",keywords:"stream sharing notifications voice custom sound alert"},
  {section:"notifications",target:"streamViewerSoundsSetting",label:"Stream viewer sounds",keywords:"screen share watch join leave notification"},
  {section:"notifications",target:"settingsRoomSounds", label:"Room sounds", keywords:"notifications voice join leave disconnect custom sound"},
  {section:"notifications", target:"settingsChatNotifications", label:"Chat notifications", keywords:"mute individual conversation sound"},
  {section:"privacy", target:"privacySettingsView", label:"Chat encryption and recovery files", keywords:"privacy security backup restore keys e2ee"},
  {section:"privacy", target:"privacyDetailsSection", label:"Encryption details", keywords:"privacy security fingerprint metadata trust model"},
  {section:"devices", target:"settingsDevices", label:"Devices", keywords:"account linked revoke access sign out sessions"},
  {section:"devices", target:"settingsAccountInvite", label:"Invite a friend", keywords:"devices account create invite"},
  {section:"server", target:"serverNameField", label:"Server name", keywords:"identity rename settings"},
  {section:"server", target:"settingsServerRoles", label:"People and roles", keywords:"server admin permissions members"},
  {section:"server", target:"newServerCategoryName", label:"Chat categories", keywords:"server organize channel create"},
  {section:"server", target:"newServerChannelName", label:"Text channels", keywords:"server chat create dedicated"},
  {section:"server", target:"settingsServerRooms", label:"Voice rooms", keywords:"server rename delete category"}
]

function search(query, canManageServer, hasPanel) {
  var words = String(query).toLowerCase().trim().split(/\s+/).filter(function(word) { return !!word })
  if (!words.length) return []
  return entries.filter(function(entry) {
    if (entry.section === "server" && !canManageServer || entry.panel && !hasPanel) return false
    var text = (entry.label + " " + entry.section + " " + entry.keywords).toLowerCase()
    return words.every(function(word) { return text.indexOf(word) >= 0 })
  })
}

function sectionLabel(section) {
  return {soundboard:"Soundboard",updates:"Updates",profile:"Profile",media:"Audio",video:"Video",appearance:"Appearance",notifications:"Notifications",privacy:"Privacy",devices:"Devices",server:"Server"}[section] || section
}
