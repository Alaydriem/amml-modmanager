namespace   = "amml"
name        = "modmanager"
entry       = "amml_modmanager.dll"
api_version = 2
entries = [
  # required: the manager refuses to enable without them
  "ClientInstance.update",
  "MouseDevice.feed",
  "HIDControllerGameCoreDesktop.onKeyDown",
  "HIDControllerGameCoreDesktop.onKeyUp",
  "MinecraftGame.grabMouse",
  "IMinecraftGame.releaseMouse",
  "IMinecraftGame.grabMouse",
  "ClientInstance.getLocalPlayer",
  "ClientInstance.isWorldActive",
  "ClientInstance.isShowingMenu",
  # optional: each one missing turns off only its own feature
  "LocalPlayer.applyTurnDelta",
  "ClientInstance.onPlayerLoaded",
  "ClientExitLevel.hook",
  "ClientInstance.getTopScreenName",
  "ClientInstance.getOptions",
  "IOptionRegistry.setHideGUI",
  "IOptionRegistry.getHideHud",
]
