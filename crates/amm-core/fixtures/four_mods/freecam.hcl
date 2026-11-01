namespace   = "alaydriem"
name        = "freecam"
display_name = "Freecam"
version      = "0.1.0"
author       = "Alaydriem"
config_path  = "config.hcl"

input_passthrough = [27, 112]

claim "Camera" {
  mode   = "Shared"
  reason = "Flies the camera while freecam is on"
}
claim "Input" {
  mode   = "Shared"
  reason = "Flight"
}
claim "Hud" {
  mode   = "Shared"
  reason = "Hides the vanilla HUD"
  hide_vanilla_hud = true
}
claim "Overlay" {
  mode   = "Shared"
  reason = "Strip and legend"
}
claim "WorldDraw" {
  mode   = "Shared"
  reason = "Path markers"
}
keybind "toggle" {
  label       = "Toggle freecam"
  default     = "Ctrl+P"
  capability  = "Camera"
  scope       = "Global"
  config_path = "keys.toggle"
}

keybind "cycle_mode" {
  label       = "Cycle mode"
  default     = "Ctrl+LeftBracket"
  capability  = "Camera"
  scope       = "Global"
  config_path = "keys.cycle_mode"
}

keybind "toggle_path_markers" {
  label       = "Toggle path markers"
  default     = "Ctrl+B"
  capability  = "WorldDraw"
  scope       = "Global"
  config_path = "keys.toggle_path_markers"
}

keybind "toggle_input" {
  label       = "Toggle input capture"
  default     = "Ctrl+E"
  capability  = "Input"
  scope       = "Global"
  config_path = "keys.toggle_input"
}

keybind "clear_path" {
  label       = "Clear path"
  default     = "Delete"
  capability  = "WorldDraw"
  scope       = "Global"
  config_path = "keys.clear_path"
}

keybind "fly_forward" {
  label       = "Fly forward"
  default     = "W"
  capability  = "Input"
  scope       = "WhileHeld"
  config_path = "keys.fly_forward"
}

keybind "fly_left" {
  label       = "Fly left"
  default     = "A"
  capability  = "Input"
  scope       = "WhileHeld"
  config_path = "keys.fly_left"
}

keybind "fly_back" {
  label       = "Fly back"
  default     = "S"
  capability  = "Input"
  scope       = "WhileHeld"
  config_path = "keys.fly_back"
}

keybind "fly_right" {
  label       = "Fly right"
  default     = "D"
  capability  = "Input"
  scope       = "WhileHeld"
  config_path = "keys.fly_right"
}

keybind "fly_up" {
  label       = "Fly up"
  default     = "Space"
  capability  = "Input"
  scope       = "WhileHeld"
  config_path = "keys.fly_up"
}

keybind "fly_down" {
  label       = "Fly down"
  default     = "Shift"
  capability  = "Input"
  scope       = "WhileHeld"
  config_path = "keys.fly_down"
}

keybind "fly_fast" {
  label       = "Fly fast"
  default     = "Ctrl"
  capability  = "Input"
  scope       = "WhileHeld"
  config_path = "keys.fly_fast"
}

section "keys" {
  title = "Keys"

  field "keys.toggle" {
    label   = "Toggle freecam"
    kind    = "chord"
    default = "Ctrl+P"
    apply   = "Live"
  }

  field "keys.cycle_mode" {
    label   = "Cycle mode"
    kind    = "chord"
    default = "Ctrl+LeftBracket"
    apply   = "Live"
  }

  field "keys.toggle_path_markers" {
    label   = "Toggle path markers"
    kind    = "chord"
    default = "Ctrl+B"
    apply   = "Live"
  }

  field "keys.toggle_input" {
    label   = "Toggle input capture"
    kind    = "chord"
    default = "Ctrl+E"
    apply   = "Live"
  }

  field "keys.clear_path" {
    label   = "Clear path"
    kind    = "chord"
    default = "Delete"
    apply   = "Live"
  }

  field "keys.fly_forward" {
    label   = "Fly forward"
    kind    = "chord"
    default = "W"
    apply   = "Live"
  }

  field "keys.fly_left" {
    label   = "Fly left"
    kind    = "chord"
    default = "A"
    apply   = "Live"
  }

  field "keys.fly_back" {
    label   = "Fly back"
    kind    = "chord"
    default = "S"
    apply   = "Live"
  }

  field "keys.fly_right" {
    label   = "Fly right"
    kind    = "chord"
    default = "D"
    apply   = "Live"
  }

  field "keys.fly_up" {
    label   = "Fly up"
    kind    = "chord"
    default = "Space"
    apply   = "Live"
  }

  field "keys.fly_down" {
    label   = "Fly down"
    kind    = "chord"
    default = "Shift"
    apply   = "Live"
  }

  field "keys.fly_fast" {
    label   = "Fly fast"
    kind    = "chord"
    default = "Ctrl"
    apply   = "Live"
  }
}
