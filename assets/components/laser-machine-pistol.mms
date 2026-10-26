import { laser_burst } from "./laser.mms"

// Device state belongs to this pistol, independently of the hand holding it.
export fn laser_machine_pistol(options) {
    let device_state = { firing = false cancelled = false bursts_fired = 0 ammunition = null }
    options.device_state = device_state
    return laser_burst(options)
}
