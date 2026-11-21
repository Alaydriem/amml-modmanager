namespace   = "amml"
name        = "modmanager"
entry       = "amml_modmanager.dll"
api_version = 3

uses {
  feature "host" {
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
    event "mods.loaded" {
      version  = 1
      required = true
    }
    area "client.read" {
      version  = 1
      required = true
    }
    event "loader.shutdown" {
      version  = 1
      required = false
    }
    area "client.top_screen" {
      version  = 2
      required = false
    }
  }
  feature "overlay" {
    event "overlay.frame_present" {
      version  = 1
      required = true
    }
    event "overlay.swap_chain_resize" {
      version  = 1
      required = true
    }
    event "overlay.swap_chain_resized" {
      version  = 1
      required = true
    }
    event "overlay.surface_lost" {
      version  = 1
      required = true
    }
    event "window.pointer" {
      version  = 1
      required = true
    }
    event "window.char" {
      version  = 1
      required = true
    }
    event "window.focus" {
      version  = 1
      required = true
    }
    event "window.size" {
      version  = 1
      required = true
    }
    event "window.changed" {
      version  = 1
      required = true
    }
    event "window.cursor" {
      version  = 1
      required = true
    }
    area "window.read" {
      version  = 1
      required = true
    }
    area "overlay.surface" {
      version  = 1
      required = true
    }
    event "overlay.frame_presented" {
      version  = 1
      required = false
    }
    event "overlay.render_device_reset" {
      version  = 1
      required = false
    }
    area "window.request_size" {
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
    event "input.mouse" {
      version  = 1
      required = false
    }
    event "client.mouse_grab" {
      version  = 1
      required = false
    }
  }
  feature "hud" {
    area "client.hud" {
      version  = 1
      required = false
    }
    area "client.mouse" {
      version  = 1
      required = false
    }
  }
}

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
