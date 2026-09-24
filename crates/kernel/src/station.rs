/// Same card → station map as POSIX `floor_station`.
pub fn floor_station(card: &str) -> &'static str {
    match card {
        c if c.starts_with("STOP-ASK")
            || c.starts_with("ESCALATE")
            || c.starts_with("INDEPENDENCE_UNAVAILABLE") =>
        {
            "ANDON"
        }
        "DONE" => "DONE",
        c if c.starts_with("CLOSED") => "DONE",
        "NEXT INTAKE" | "NEXT CAST" | "NEXT RESEARCH" | "NEXT REPO" | "NEXT SPEC" => "SHAPE",
        "NEXT MAP" | "NEXT RUN scout" => "DESIGN",
        "NEXT RUN reviewer" | "NEXT CLOSE" => "INSPECT",
        "NEXT RECORD PRE-FALSIFY"
        | "NEXT RUN maker-falsify"
        | "NEXT RED"
        | "NEXT RUN maker-build"
        | "NEXT BUILT"
        | "NEXT GREEN" => "BUILD",
        c if c.starts_with("NEXT SLICE ") => "BUILD",
        // Exact `DONE` is matched above; this covers `DONE — …`.
        c if c.starts_with("DONE") => "DONE",
        c if c.starts_with("NEXT CONFIGURE")
            || c.starts_with("NEXT INVESTIGATE")
            || c.starts_with("NEXT PROPOSE")
            || c.starts_with("NEXT PLAN") =>
        {
            "SHAPE"
        }
        c if c.starts_with("NEXT EXECUTE") => "BUILD",
        c if c.starts_with("NEXT REVIEW") => "INSPECT",
        // After STOP-ASK so `STOP-ASK WAIT …` stays ANDON.
        c if c.starts_with("WAIT ") => "WAIT",
        c => {
            if c.contains("MAP-HUMAN") || c.contains("ARCH") {
                "ANDON"
            } else if c.contains("SLICE")
                || c.contains("maker-")
                || c.contains("RED")
                || c.contains("BUILT")
                || c.contains("GREEN")
            {
                "BUILD"
            } else if c.contains("MAP") {
                "DESIGN"
            } else {
                "SHAPE"
            }
        }
    }
}
