use crate::run_from;
use tempfile::tempdir;

#[test]
fn gen_rejects_quadratic_outputs_before_writing() {
    for (task, distance, projected_qubits) in [
        ("rotated_memory_z", "257", "132097"),
        ("unrotated_memory_z", "182", "131769"),
    ] {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_from(
            [
                "stab",
                "gen",
                "--code",
                "surface_code",
                "--task",
                task,
                "--distance",
                distance,
                "--rounds",
                "1",
            ],
            std::io::empty(),
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(status, 1, "task={task}");
        assert!(stdout.is_empty(), "task={task}");
        let stderr = String::from_utf8(stderr).expect("UTF-8 stderr");
        assert!(stderr.contains(projected_qubits), "{stderr}");
        assert!(stderr.contains("current limit is 131072"), "{stderr}");
    }

    let temp_dir = tempdir().expect("temporary directory");
    let output = temp_dir.path().join("oversized.stim");
    let output_arg = output.to_str().expect("UTF-8 temporary path");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "gen",
            "--code",
            "color_code",
            "--task",
            "memory_xyz",
            "--distance",
            "343",
            "--rounds",
            "2",
            "--out",
            output_arg,
        ],
        std::io::empty(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert!(stdout.is_empty());
    assert!(
        !output.exists(),
        "rejection must not create a partial output"
    );
    let stderr = String::from_utf8(stderr).expect("UTF-8 stderr");
    assert!(stderr.contains("132355"), "{stderr}");
    assert!(stderr.contains("current limit is 131072"), "{stderr}");
}
