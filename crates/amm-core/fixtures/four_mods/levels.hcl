namespace   = "alaydriem"
name        = "levels"
display_name = "Light Level"
version      = "0.1.0"
author       = "Alaydriem"
config_path  = "config.hcl"

claim "WorldDraw" {
  mode   = "Shared"
  reason = "Light level overlay"
}
keybind "toggle" {
  label       = "Toggle light levels"
  default     = "Ctrl+L"
  capability  = "WorldDraw"
  scope       = "Global"
  config_path = "keys.toggle"
}

section "keys" {
  title = "Keys"

  field "keys.toggle" {
    label   = "Toggle light levels"
    kind    = "chord"
    default = "Ctrl+L"
    apply   = "Live"
  }
}
