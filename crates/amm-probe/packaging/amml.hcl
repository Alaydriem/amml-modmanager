namespace   = "amml"
name        = "managerprobe"
entry       = "amml_managerprobe.dll"
api_version = 3
uses {
  feature "core" {
    event "mods.loaded" {
      version  = 1
      required = true
    }
    event "client.tick" {
      version  = 1
      required = true
    }
    event "world.joined" {
      version  = 1
      required = true
    }
    event "world.left" {
      version  = 1
      required = true
    }
    event "registry.provider_ready" {
      version  = 1
      required = true
    }
    event "registry.provider_gone" {
      version  = 1
      required = true
    }
    event "loader.shutdown" {
      version  = 1
      required = false
    }
  }
  feature "input" {
    event "input.key" {
      version  = 1
      required = true
    }
    area "input.lease" {
      version  = 1
      required = true
    }
    event "input.player_move" {
      version  = 1
      required = false
    }
    event "input.look_delta" {
      version  = 1
      required = false
    }
    event "input.mouse" {
      version  = 1
      required = false
    }
    event "input.gamepad_poll" {
      version  = 1
      required = false
    }
    event "input.injected_released" {
      version  = 2
      required = false
    }
    event "input.lease_suspended" {
      version  = 1
      required = false
    }
    event "input.lease_resumed" {
      version  = 1
      required = false
    }
    event "input.lease_lost" {
      version  = 1
      required = false
    }
    area "input.key_inject" {
      version  = 1
      required = false
    }
    area "input.mouse_inject" {
      version  = 1
      required = false
    }
    area "input.turn" {
      version  = 1
      required = false
    }
    area "input.move" {
      version  = 1
      required = false
    }
  }
  feature "overlay" {
    event "overlay.screenshot_taken" {
      version  = 1
      required = false
    }
    event "overlay.screenshot_written" {
      version  = 1
      required = false
    }
    area "overlay.screenshot" {
      version  = 1
      required = false
    }
  }
}
