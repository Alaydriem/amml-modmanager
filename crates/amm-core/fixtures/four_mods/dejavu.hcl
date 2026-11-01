namespace   = "alaydriem"
name        = "dejavu"
display_name = "DejaVu"
version      = "0.1.0"
author       = "Alaydriem"
config_path  = "config.hcl"

claim "Camera" {
  mode   = "Exclusive"
  reason = "Replay free camera"
}

claim "Input" {
  mode   = "Shared"
  reason = "Editor"
}

claim "Hud" {
  mode   = "Shared"
  reason = "Replay HUD"
}

claim "Overlay" {
  mode   = "Shared"
  reason = "Editor"
}

claim "WorldDraw" {
  mode   = "Shared"
  reason = "Replay markers"
}

