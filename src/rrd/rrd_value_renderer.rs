//! Helpers for rendering values in RRD graphs.

/// Render CPU usage in percent. `v` is multiplied by 100 to get the percent value.
pub fn render_cpu_usage(v: &f64) -> String {
    if v.is_finite() {
        format!("{:.1}%", v * 100.0)
    } else {
        v.to_string()
    }
}

/// Render server load value.
pub fn render_load(v: &f64) -> String {
    if v.is_finite() {
        format!("{:.2}", v)
    } else {
        v.to_string()
    }
}

/// Render a byte value.
pub fn render_bytes(v: &f64) -> String {
    if v.is_finite() {
        proxmox_human_byte::HumanByte::from(*v as u64).to_string()
    } else {
        v.to_string()
    }
}

/// Render bandwidth.
pub fn render_bandwidth(v: &f64) -> String {
    if v.is_finite() {
        let bytes = proxmox_human_byte::HumanByte::from(*v as u64);
        format!("{bytes}/s")
    } else {
        v.to_string()
    }
}

/// Render pressure stall value.
pub fn render_pressure(v: &f64) -> String {
    if v.is_finite() {
        format!("{:.1}%", v)
    } else {
        v.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_cpu_usage() {
        assert_eq!(render_cpu_usage(&0.532), "53.2%");
        assert_eq!(render_cpu_usage(&f64::NAN), "NaN");
    }

    #[test]
    fn test_render_load() {
        assert_eq!(render_load(&0.538), "0.54");
        assert_eq!(render_load(&f64::NAN), "NaN");
    }

    #[test]
    fn test_render_bytes() {
        assert_eq!(render_bytes(&(1024f64 * 1024f64)), "1 MiB");
        assert_eq!(render_bytes(&(1254f64 * 1024f64)), "1.225 MiB");
        assert_eq!(render_bytes(&f64::NAN), "NaN");
    }

    #[test]
    fn test_render_bandwidth() {
        assert_eq!(render_bandwidth(&(1024f64 * 1024f64)), "1 MiB/s");
        assert_eq!(render_bandwidth(&(1254f64 * 1024f64)), "1.225 MiB/s");
        assert_eq!(render_bandwidth(&f64::NAN), "NaN");
    }

    #[test]
    fn test_render_pressure() {
        assert_eq!(render_pressure(&0.538), "0.5%");
        assert_eq!(render_pressure(&f64::NAN), "NaN");
    }
}
