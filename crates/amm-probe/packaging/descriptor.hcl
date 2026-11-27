namespace    = "amml"
name         = "managerprobe"
display_name = "Manager Probe"
version      = "0.1.0"
author       = "Charles R. Portwood II"
config_path  = "config.hcl"

# F8, the probe key, reaches the game and the manager while the probe's Input lease blocks the other real keys
input_passthrough = [119]

claim "Input" {
  mode   = "Shared"
  reason = "Injects keys, the mouse, movement and turns for the in-game checks"
}

claim "Hud" {
  mode             = "Shared"
  reason           = "Proves the HUD lease and the hidden vanilla HUD"
  hide_vanilla_hud = true
}

claim "Overlay" {
  mode   = "Shared"
  reason = "Draws one magenta square through the manager"
}

keybind "act" {
  label       = "Probe key"
  default     = "F8"
  capability  = "Hud"
  scope       = "Global"
  config_path = "keys.act"
}

section "probe" {
  title = "Probe"

  field "keys.act" {
    label   = "Probe key"
    kind    = "chord"
    default = "F8"
    apply   = "Live"
  }

  field "probe.count" {
    label   = "Probe count"
    kind    = { int = { min = 0, max = 100, step = 1 } }
    default = 1
    apply   = "Live"
  }
}
