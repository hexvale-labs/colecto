# Tauri looks plugins up by class name and deserializes @InvokeArg payloads via reflection.
-keep class app.colecto.mobile.NativePlugin { *; }
-keep class app.colecto.mobile.PressArgs { *; }
-keep class app.colecto.mobile.SecretIdArgs { *; }
-keep class app.colecto.mobile.SecretSetArgs { *; }
