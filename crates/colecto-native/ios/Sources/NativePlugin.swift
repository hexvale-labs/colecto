import SwiftRs
import Tauri
import UIKit
import WebKit

class PressArgs: Decodable {
  let kind: String?
}

/// Press feedback for Colecto's page. iOS controls do not click, so only switches and
/// choices get a selection tick. The back swipe is handled by the page itself.
class NativePlugin: Plugin {
  @objc public func press(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(PressArgs.self)
    if args.kind == "toggle" {
      DispatchQueue.main.async {
        UISelectionFeedbackGenerator().selectionChanged()
      }
    }
    invoke.resolve()
  }
}

@_cdecl("init_plugin_colecto_native")
func initPlugin() -> Plugin {
  return NativePlugin()
}
