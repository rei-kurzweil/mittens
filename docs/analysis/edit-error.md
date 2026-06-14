--- a/src/engine/ecs/system/editor_paint_system.rs
+++ b/src/engine/ecs/system/editor_paint_system.rs
@@ -366,12 +366,7 @@ fn bootstrap_paint_state(
                 },
             );
         }
-        state_str = format!("{state:?}");
     }
-    eprintln!(
-        "🎨🖌 paint_debug bootstrap_paint_state done → {}",
-        state_str
-    );
 }
 
 fn label_from_component_id(world: &World, id: ComponentId) -> Option<String> {
@@ -2306,3 +2301,7 @@ mod tests {
         );
     }
 }
+ subtree"
+        );
+    }
+}

