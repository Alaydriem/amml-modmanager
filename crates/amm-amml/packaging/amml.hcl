namespace   = "amml"
name        = "modmanager"
entry       = "amml_modmanager.dll"
api_version = 2
amml_version = "26.60.6"
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
  "GameControllerHandler_GameCore.refresh",
  "ClientInstance.onPlayerLoaded",
  "ClientExitLevel.hook",
  "ClientInstance.getTopScreenName",
  "ClientInstance.getOptions",
  "IOptionRegistry.setHideGUI",
  "IOptionRegistry.getHideHud",
]

listing {
  title       = "Alaydriem's Mod Manager"
  summary     = "One panel for every AMML mod: keybinds, settings, key clashes, and turning mods on or off."
  description = <<-EOT
    AMML's mod manager lets you manage, toggle, and configure your mods without leaving the game.
  EOT
  authors     = ["Charles R. Portwood II"]
  license     = "BSD-3-Clause"
  source      = "https://github.com/alaydriem/amml-modmanager"
  homepage    = "https://github.com/alaydriem/amml-modmanager#readme"
  tags        = ["manager", "keybinds", "settings", "overlay", "tools"]
  icon        = "icon.png"
}
