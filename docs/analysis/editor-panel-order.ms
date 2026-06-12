@@ -1478,11 +1478,11 @@ impl EditorInspectorSystemStopgapMmsReconciler {
                 mount_name: PANEL_LAYOUT_MOUNT_NAME.to_string(),
                 layout_name: PANEL_LAYOUT_ROOT_NAME.to_string(),
                 children: vec![
+                    editor_settings_panel,
                     paint_panel,
-                    asset_panel,
-                    world_panel,
                     grid_panel,
-                    editor_settings_panel,
+                    world_panel,
+                    asset_panel,
                 ],
             },
         ) {

