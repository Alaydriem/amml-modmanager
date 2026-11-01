namespace   = "alaydriem"
name        = "minimap"
display_name = "Minimap"
version      = "0.1.0"
author       = "Alaydriem"
config_path  = "config.hcl"

claim "Input" {
  mode   = "Shared"
  reason = "World map open"
}
claim "Hud" {
  mode   = "Shared"
  reason = "Corner map"
}
claim "Overlay" {
  mode   = "Shared"
  reason = "World map"
}
keybind "toggle_world_map" {
  label       = "Toggle world map"
  default     = "M"
  capability  = "Overlay"
  scope       = "Global"
  config_path = "keys.toggle_world_map"
}

keybind "toggle_minimap" {
  label       = "Toggle minimap"
  default     = "Ctrl+M"
  capability  = "Hud"
  scope       = "Global"
  config_path = "keys.toggle_minimap"
}

keybind "zoom_in" {
  label       = "Zoom in"
  default     = "Equals"
  capability  = "Hud"
  scope       = "Global"
  config_path = "keys.zoom_in"
}

keybind "zoom_out" {
  label       = "Zoom out"
  default     = "Minus"
  capability  = "Hud"
  scope       = "Global"
  config_path = "keys.zoom_out"
}

keybind "toggle_north_lock" {
  label       = "Toggle north lock"
  default     = "N"
  capability  = "Hud"
  scope       = "Global"
  config_path = "keys.toggle_north_lock"
}

keybind "cycle_shadow" {
  label       = "Cycle shadow style"
  default     = "K"
  capability  = "Hud"
  scope       = "Global"
  config_path = "keys.cycle_shadow"
}

keybind "cycle_shadow_reverse" {
  label       = "Cycle shadow style backwards"
  default     = "Ctrl+K"
  capability  = "Hud"
  scope       = "Global"
  config_path = "keys.cycle_shadow_reverse"
}

section "keys" {
  title = "Keys"

  field "keys.toggle_world_map" {
    label   = "Toggle world map"
    kind    = "chord"
    default = "M"
    apply   = "Live"
  }

  field "keys.toggle_minimap" {
    label   = "Toggle minimap"
    kind    = "chord"
    default = "Ctrl+M"
    apply   = "Live"
  }

  field "keys.zoom_in" {
    label   = "Zoom in"
    kind    = "chord"
    default = "Equals"
    apply   = "Live"
  }

  field "keys.zoom_out" {
    label   = "Zoom out"
    kind    = "chord"
    default = "Minus"
    apply   = "Live"
  }

  field "keys.toggle_north_lock" {
    label   = "Toggle north lock"
    kind    = "chord"
    default = "N"
    apply   = "Live"
  }

  field "keys.cycle_shadow" {
    label   = "Cycle shadow style"
    kind    = "chord"
    default = "K"
    apply   = "Live"
  }

  field "keys.cycle_shadow_reverse" {
    label   = "Cycle shadow style backwards"
    kind    = "chord"
    default = "Ctrl+K"
    apply   = "Live"
  }
}
