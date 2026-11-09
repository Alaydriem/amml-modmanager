namespace    = "alaydriem"
name         = "headless"
display_name = "Headless"
version      = "0.1.0"
author       = "Alaydriem"
config_path  = "config.hcl"

input_passthrough = [87, 114]

claim "Input" {
  mode              = "Exclusive"
  reason            = "Drives the game without a person at the keyboard"
  full_block        = true
  pass_pointer_motion = true
}
