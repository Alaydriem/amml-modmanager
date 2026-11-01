namespace   = "alaydriem"
name        = "demo"
display_name = "Demo"
version      = "0.1.0"
author       = "Alaydriem"
config_path  = "config.hcl"

claim "Camera" {
  mode   = "Shared"
  reason = "Takes the camera while the take camera key is toggled on"
}
claim "Input" {
  mode   = "Shared"
  reason = "Reads keys while the demo window is open"
  block_player_turn = true
}
claim "Overlay" {
  mode   = "Shared"
  reason = "Shows the demo window"
}
claim "Hud" {
  mode   = "Shared"
  reason = "Shows the demo card"
}

keybind "toggle_card" {
  label       = "Toggle card"
  default     = "Alt+1"
  capability  = "Hud"
  scope       = "Global"
  config_path = "keys.toggle_card"
}

keybind "take_camera" {
  label       = "Take or release the camera"
  default     = "Alt+2"
  capability  = "Camera"
  scope       = "Global"
  config_path = "keys.take_camera"
}

keybind "open_window" {
  label       = "Open or close the window"
  default     = "Alt+3"
  capability  = "Overlay"
  scope       = "Global"
  config_path = "keys.open_window"
}

keybind "screenshot" {
  label       = "Take screenshot"
  default     = "Alt+4"
  capability  = "Hud"
  scope       = "Global"
  config_path = "keys.screenshot"
}

keybind "screenshot_layers" {
  label       = "Take screenshot with mod layers"
  default     = "Alt+5"
  capability  = "Hud"
  scope       = "Global"
  config_path = "keys.screenshot_layers"
}

section "card" {
  title = "Card"

  field "card.compact" {
    label   = "Compact card"
    help    = "Hides the bars and shows the numbers only"
    kind    = "bool"
    default = false
    apply   = "Live"
  }

  field "card.rows" {
    label   = "Rows"
    help    = "Number of marks the card draws"
    kind    = { int = { min = 1, max = 32, step = 1 } }
    default = 8
    apply   = "Live"
  }

  field "card.opacity" {
    label   = "Opacity"
    help    = "How solid the card background is"
    kind    = { float = { min = 0.0, max = 1.0, step = 0.05 } }
    default = 0.9
    apply   = "Live"
  }

  field "card.corner" {
    label   = "Corner"
    help    = "Where the card sits on the screen"
    kind    = { enum = { options = [{ value = "top_left", label = "Top left" }, { value = "top_right", label = "Top right" }, { value = "bottom_right", label = "Bottom right" }] } }
    default = "top_left"
    apply   = "Live"
  }

  field "card.accent" {
    label   = "Accent"
    help    = "Colour of the marks"
    kind    = { color = { alpha = false, encoding = "HexString" } }
    default = "#8B7CFF"
    apply   = "Live"
  }
}

section "debug" {
  title = "Debug"

  field "debug_second_mod" {
    label   = "Register a second mod"
    help    = "Also registers alaydriem-demo-b, which claims the camera exclusively, to show a suspension, a denial and a key clash"
    kind    = "bool"
    default = false
    apply   = "Restart"
  }
}

section "keys" {
  title = "Keys"

  field "keys.toggle_card" {
    label   = "Toggle card"
    kind    = "chord"
    default = "Alt+1"
    apply   = "Live"
  }

  field "keys.take_camera" {
    label   = "Take or release the camera"
    kind    = "chord"
    default = "Alt+2"
    apply   = "Live"
  }

  field "keys.open_window" {
    label   = "Open or close the window"
    kind    = "chord"
    default = "Alt+3"
    apply   = "Live"
  }

  field "keys.screenshot" {
    label   = "Take screenshot"
    kind    = "chord"
    default = "Alt+4"
    apply   = "Live"
  }

  field "keys.screenshot_layers" {
    label   = "Take screenshot with mod layers"
    kind    = "chord"
    default = "Alt+5"
    apply   = "Live"
  }
}
