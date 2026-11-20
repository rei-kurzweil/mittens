use android_activity::{AndroidApp, InputStatus, MainEvent, PollEvent};

pub(super) fn run(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_tag("mitten-droid")
            .with_max_level(log::LevelFilter::Info),
    );
    std::panic::set_hook(Box::new(|panic| log::error!("host_panic: {panic}")));
    forward_stdio_to_logcat();

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
    #[cfg(feature = "xr-probe")]
    let mut worker = crate::xr_host::Worker::new(app.internal_data_path());
    #[cfg(not(feature = "xr-probe"))]
    log::info!("lifecycle-only host; engine and OpenXR/Vulkan are not started");

    // This thread owns activity polling; the optional worker owns all XR and
    // Vulkan calls. It is joined before Android releases its JVM/context.
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
                    MainEvent::Resume { .. } => {
                        log::info!("lifecycle Resume");
                        #[cfg(feature = "xr-probe")]
                        worker.resume();
                    }
                    MainEvent::Pause => {
                        log::info!("lifecycle Pause");
                        #[cfg(feature = "xr-probe")]
                        worker.pause();
                    }
                    MainEvent::SaveState { .. } => log::info!("lifecycle SaveState"),
                    event => log::info!("lifecycle {event:?}"),
                }
            }
        });
    }

    #[cfg(feature = "xr-probe")]
    drop(worker);
    log::info!("host_exit clean; returning from android_main");
}

/// Engine diagnostics and MMS `print` write to stdout/stderr, which Android
/// discards. Forward both into logcat (tag `mitten-droid`, prefix `stdio`).
fn forward_stdio_to_logcat() {
    use std::io::{BufRead, BufReader};
    use std::os::fd::FromRawFd;
    let mut fds = [0; 2];
    // SAFETY: plain fd plumbing at startup, before any other thread writes stdio.
    unsafe {
        if libc::pipe(fds.as_mut_ptr()) != 0 {
            log::error!("stdio pipe failed; stdout/stderr stay discarded");
            return;
        }
        libc::dup2(fds[1], libc::STDOUT_FILENO);
        libc::dup2(fds[1], libc::STDERR_FILENO);
        libc::close(fds[1]);
    }
    // SAFETY: fds[0] is a freshly created pipe read end owned only here.
    let reader = unsafe { std::fs::File::from_raw_fd(fds[0]) };
    let spawned = std::thread::Builder::new()
        .name("stdio-logcat".into())
        .spawn(move || {
            for line in BufReader::new(reader).lines() {
                match line {
                    Ok(line) => log::info!("stdio {line}"),
                    Err(_) => break,
                }
            }
        });
    if let Err(error) = spawned {
        log::error!("stdio forwarding thread failed: {error}");
    }
}
