use android_activity::{AndroidApp, InputStatus, MainEvent, PollEvent};

pub(super) fn run(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_tag("mitten-droid")
            .with_max_level(log::LevelFilter::Info),
    );
    std::panic::set_hook(Box::new(|panic| log::error!("host_panic: {panic}")));

    log::info!(
        "host_start version={} pid={} thread={:?}",
        env!("CARGO_PKG_VERSION"),
        std::process::id(),
        std::thread::current().id(),
    );

    // android-activity initializes ndk-context and attaches this thread to the
    // JVM before calling android_main. Do not initialize or release it twice.
    let context = ndk_context::android_context();
    assert!(!context.vm().is_null() && !context.context().is_null());
    assert_eq!(context.vm(), app.vm_as_ptr());
    assert_eq!(context.context(), app.activity_as_ptr());
    log::info!("android_context ready; activity and JVM match NativeActivity");
    log::info!("lifecycle-only host; engine and OpenXR/Vulkan are not started");

    // This thread owns activity polling. The later XR driver must coordinate
    // with it; there are no host worker threads to stop or join in this stage.
    let mut destroy_requested = false;
    while !destroy_requested {
        // Wait for an event instead of running a frame loop while idle/paused.
        app.poll_events(None, |event| {
            if let PollEvent::Main(event) = event {
                match event {
                    MainEvent::InputAvailable => match app.input_events_iter() {
                        Ok(mut input) => {
                            // Finish every queued event, allowing Android's
                            // default handling (including Back) to run.
                            while input.next(|_| InputStatus::Unhandled) {}
                        }
                        Err(error) => log::error!("input_events_iter failed: {error}"),
                    },
                    MainEvent::Destroy => {
                        log::info!("lifecycle Destroy");
                        destroy_requested = true;
                    }
                    // Their Debug output includes the complete activity state;
                    // keep lifecycle diagnostics readable instead.
                    MainEvent::Resume { .. } => log::info!("lifecycle Resume"),
                    MainEvent::SaveState { .. } => log::info!("lifecycle SaveState"),
                    event => log::info!("lifecycle {event:?}"),
                }
            }
        });
    }

    log::info!("host_exit clean; returning from android_main");
}
