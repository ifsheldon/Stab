//! Compile-time and session storage admission of detection plans and sessions.

use super::*;

fn budget(bytes: impl TryInto<u64>) -> MemoryBudget {
    MemoryBudget::from_bytes(bytes.try_into().ok().expect("test budget fits u64"))
}

#[test]
fn compilation_rejects_limits_before_session_or_sink_work() {
    let circuit = circuit("M 0 1\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n");
    let conversion_error = MeasurementToDetectionCompiler {
        memory_budget: budget(0_u64),
        ..MeasurementToDetectionCompiler::default()
    }
    .compile(&circuit)
    .expect_err("reject conversion plan beyond its record limit");
    assert_matches!(&conversion_error, DetectionCompileError::InvalidCircuit(_));
    assert!(conversion_error.to_string().contains("memory budget"));
    let sampling_error = DetectionSamplingCompiler {
        memory_budget: budget(0_u64),
        ..DetectionSamplingCompiler::default()
    }
    .compile(&circuit)
    .expect_err("reject detection sampling plan beyond its record limit");
    assert_matches!(&sampling_error, DetectionCompileError::InvalidCircuit(_));
    assert!(sampling_error.to_string().contains("memory budget"));
}

#[test]
fn direct_frame_compilation_charges_executable_targets_before_materialization() {
    let targets = (0..256)
        .map(|qubit| format!("X{qubit}"))
        .collect::<Vec<_>>()
        .join(" ");
    let metadata = "tag".repeat(1_024);
    let tagged = circuit(&format!("OBSERVABLE_INCLUDE[{metadata}](0) {targets}\n"));
    let untagged = circuit(&format!("OBSERVABLE_INCLUDE(0) {targets}\n"));
    let compiler = DetectionSamplingCompiler::default();
    let tagged_plan = compiler
        .compile(&tagged)
        .expect("compile tagged direct frame");
    let untagged_plan = compiler
        .compile(&untagged)
        .expect("compile untagged direct frame");
    let exact_bytes = direct_compiled_bytes(&tagged_plan);
    assert_eq!(
        exact_bytes,
        direct_compiled_bytes(&untagged_plan),
        "nonsemantic tags must not be retained by the private executable"
    );

    DetectionSamplingCompiler {
        memory_budget: budget(exact_bytes),
        ..DetectionSamplingCompiler::default()
    }
    .compile(&tagged)
    .expect("accept exact combined direct-plan byte boundary");
    let error = DetectionSamplingCompiler {
        memory_budget: budget(exact_bytes - 1),
        ..DetectionSamplingCompiler::default()
    }
    .compile(&tagged)
    .expect_err("reject the first byte above the direct-plan boundary");
    let DetectionCompileError::InvalidCircuit(DetectionError::ResourceLimit(resource)) = error
    else {
        panic!("direct-plan byte rejection must remain typed");
    };
    assert_eq!(resource.kind(), DetectionResourceKind::CompiledBytes);
    assert_eq!(resource.actual(), exact_bytes);
    assert_eq!(resource.limit(), exact_bytes - 1);
}

#[test]
fn direct_frame_compilation_admits_repeats_and_filtered_targets_before_materialization() {
    let repeated =
        circuit("REPEAT 4096 {\n M 0\n DETECTOR rec[-1]\n OBSERVABLE_INCLUDE(0) X0 rec[-1]\n}\n");
    let filtered_targets = (0..512)
        .flat_map(|index| {
            let qubit = index * 3;
            [
                format!("{qubit} sweep[{index}]"),
                format!("{} {}", qubit + 1, qubit + 2),
            ]
        })
        .collect::<Vec<_>>()
        .join(" ");
    let filtered = circuit(&format!(
        "XCZ {filtered_targets}\nOBSERVABLE_INCLUDE(0) X0\n"
    ));

    let baseline = circuit("OBSERVABLE_INCLUDE(0) X0\n");
    let baseline_exact = direct_compiled_bytes(
        &DetectionSamplingCompiler::default()
            .compile(&baseline)
            .expect("compile rejection-overhead baseline"),
    );
    let baseline_allocations = rejected_direct_frame_allocations(&baseline, baseline_exact);

    for circuit in [&repeated, &filtered] {
        let exact_plan = DetectionSamplingCompiler::default()
            .compile(circuit)
            .expect("compile exact-byte probe");
        let exact_bytes = direct_compiled_bytes(&exact_plan);
        DetectionSamplingCompiler {
            memory_budget: budget(exact_bytes),
            ..DetectionSamplingCompiler::default()
        }
        .compile(circuit)
        .expect("accept exact combined direct-plan byte boundary");

        let measured = rejected_direct_frame_allocations(circuit, exact_bytes);
        assert_eq!(
            measured.count_max, baseline_allocations.count_max,
            "aggregate byte rejection retained plan allocations concurrently"
        );
        assert_eq!(
            measured.bytes_max, baseline_allocations.bytes_max,
            "aggregate byte rejection allocated a retained plan buffer"
        );
    }
}

fn rejected_direct_frame_allocations(
    circuit: &Circuit,
    exact_bytes: u64,
) -> allocation_counter::AllocationInfo {
    let mut rejected = false;
    let measured = allocation_counter::measure(|| {
        rejected = DetectionSamplingCompiler {
            memory_budget: budget(exact_bytes - 1),
            ..DetectionSamplingCompiler::default()
        }
        .compile(circuit)
        .is_err();
    });
    assert!(rejected, "reject first byte beyond retained-plan limit");
    measured
}

fn direct_compiled_bytes(plan: &DetectionSamplingPlan) -> u64 {
    plan.inner
        .direct
        .compiled_bytes()
        .expect("compute retained direct-plan bytes")
}

#[test]
fn warmed_conversion_reuses_width_and_batch_bounded_storage() {
    let circuit = circuit(
        "H 0\nCX sweep[0] 0\nSPP X0*Z1\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    );
    let plan = MeasurementToDetectionCompiler::default()
        .compile(&circuit)
        .expect("compile sweep conversion");
    let mut session = plan
        .session(&Default::default())
        .expect("create conversion session");
    let measurements = packed(&vec![vec![false]; 64], 1);
    let sweeps = packed(&vec![vec![true]; 64], 1);
    let measurement_view = MeasurementBatchView::new(measurements.view());
    let sweep_view = MeasurementBatchView::new(sweeps.view());
    let mut sink = NullSink::default();

    let mut delivery = session
        .start_transaction(&mut sink)
        .expect("start allocation delivery");
    delivery
        .write_batch_with_sweep(measurement_view, Some(sweep_view))
        .expect("warm conversion scratch");
    let measured = allocation_counter::measure(|| {
        for _ in 0..128 {
            delivery
                .write_batch_with_sweep(measurement_view, Some(sweep_view))
                .expect("reuse conversion scratch");
        }
    });
    delivery.finish().expect("finish allocation delivery");
    assert_eq!(
        measured.count_total, 0,
        "warmed conversion allocated while reusing bounded scratch: {measured:?}"
    );
    assert_eq!(sink.shots, 64 * 129);
}

#[test]
fn session_storage_estimates_match_retained_allocations_at_word_boundaries() {
    for (detectors, observables) in [(0, 0), (1, 1), (63, 64), (64, 63), (65, 65)] {
        let mut text = String::from("M 0\n");
        for _ in 0..detectors {
            text.push_str("DETECTOR rec[-1]\n");
        }
        if observables > 0 {
            text.push_str(&format!(
                "OBSERVABLE_INCLUDE({}) rec[-1]\n",
                observables - 1
            ));
        }
        let direct = DetectionSamplingCompiler::default()
            .compile(&circuit(&text))
            .expect("compile direct allocation fixture");
        for &lane_width in LaneWidth::ALL {
            let mut session = None;
            let allocated = allocation_counter::measure(|| {
                session = Some(
                    direct
                        .session(&DetectionSamplingSessionOptions {
                            random: RandomPolicy::Seeded(Seed::new(1)),
                            lane_width,
                            ..Default::default()
                        })
                        .expect("allocate direct session"),
                );
            });
            assert_eq!(
                execution::direct_session_storage_bytes(&direct.inner.direct, lane_width),
                u128::try_from(allocated.bytes_current).expect("retained direct allocation"),
                "direct widths {detectors}/{observables} at {lane_width:?}"
            );
        }

        for sweep in [false, true] {
            let text = if sweep {
                format!("CX sweep[0] 0\n{text}")
            } else {
                text.clone()
            };
            let conversion = MeasurementToDetectionCompiler::default()
                .compile(&circuit(&text))
                .expect("compile conversion allocation fixture");
            // A windowed session sizes its records by the batches it converts, so its storage
            // is complete once it converted a batch of the most shots it accepts.
            let records = PackedShotBatch::zeros(MAX_BATCH_SHOTS, 1).expect("full input batch");
            let mut session = None;
            let allocated = allocation_counter::measure(|| {
                let mut created = conversion
                    .session(&Default::default())
                    .expect("allocate conversion session");
                created
                    .run(
                        MeasurementBatchView::new(records.view()),
                        None,
                        &mut NullSink::default(),
                    )
                    .expect("convert a full batch");
                session = Some(created);
            });
            assert_eq!(
                execution::conversion_session_storage_bytes(&conversion),
                u128::try_from(allocated.bytes_current).expect("retained conversion allocation"),
                "conversion widths {detectors}/{observables}, sweep={sweep}"
            );
        }
    }
}

#[test]
fn direct_session_storage_admits_the_exact_budget_and_rejects_the_first_excess_byte() {
    let fixture = circuit("M 1000\nDETECTOR rec[-1]\n");
    let probe = DetectionSamplingCompiler::default()
        .compile(&fixture)
        .expect("compile storage probe");
    let seeded = RandomPolicy::Seeded(Seed::new(1));
    let narrow = execution::direct_session_storage_bytes(&probe.inner.direct, LaneWidth::Lanes64);
    let wide = execution::direct_session_storage_bytes(&probe.inner.direct, LaneWidth::Lanes256);
    assert_eq!(
        wide,
        4 * narrow,
        "a narrow session holds a quarter of the planes"
    );
    for &lane_width in LaneWidth::ALL {
        let exact = execution::direct_session_storage_bytes(&probe.inner.direct, lane_width);
        let at_limit = DetectionSamplingCompiler {
            memory_budget: budget(exact),
            ..DetectionSamplingCompiler::default()
        }
        .compile(&fixture)
        .expect("the plan fits the session budget");
        at_limit
            .session(&DetectionSamplingSessionOptions {
                random: seeded,
                lane_width,
                ..Default::default()
            })
            .expect("admit the exact session budget");

        let over_limit = DetectionSamplingCompiler {
            memory_budget: budget(exact - 1),
            ..DetectionSamplingCompiler::default()
        }
        .compile(&fixture)
        .expect("the plan still fits one byte less");
        let mut rejected = None;
        let allocated = allocation_counter::measure(|| {
            rejected = Some(
                over_limit
                    .session(&DetectionSamplingSessionOptions {
                        random: seeded,
                        lane_width,
                        ..Default::default()
                    })
                    .expect_err("reject the first excess byte before allocation"),
            );
        });
        assert_eq!(allocated.bytes_total, 0, "{lane_width:?}");
        assert_eq!(
            rejected,
            Some(DetectionExecutionError::SessionStorageLimit {
                exceeded: refusal(exact),
            }),
            "{lane_width:?}"
        );
    }
}

#[test]
fn conversion_session_storage_admits_the_exact_budget_and_rejects_the_first_excess_byte() {
    let fixture = circuit("CX sweep[0] 1000\nM 0 0 0 0 0 0 0 0\nDETECTOR rec[-1]\n");
    let compile = |memory_budget| {
        MeasurementToDetectionCompiler {
            reference_sample_mode: ReferenceSampleMode::SkipReferenceSample,
            memory_budget,
            ..MeasurementToDetectionCompiler::default()
        }
        .compile(&fixture)
        .expect("compile conversion storage probe")
    };
    let exact = execution::conversion_session_storage_bytes(&compile(MemoryBudget::DEFAULT));
    compile(budget(exact))
        .session(&Default::default())
        .expect("admit the exact session budget");

    let over_limit = compile(budget(exact - 1));
    let mut rejected = None;
    let allocated = allocation_counter::measure(|| {
        rejected = Some(
            over_limit
                .session(&Default::default())
                .expect_err("reject the first excess byte before allocation"),
        );
    });
    assert_eq!(allocated.bytes_total, 0);
    assert_eq!(
        rejected,
        Some(DetectionExecutionError::SessionStorageLimit {
            exceeded: refusal(exact),
        })
    );
}

#[test]
fn warmed_detection_session_reuses_batch_storage() {
    let spp = (0..32)
        .map(|qubit| format!("X{qubit}"))
        .collect::<Vec<_>>()
        .join("*");
    let circuit = circuit(&format!(
        "SPP {spp}\nX_ERROR(0.25) 0\nM 0\nDETECTOR rec[-1]\n"
    ));
    let plan = DetectionSamplingCompiler::default()
        .compile(&circuit)
        .expect("compile detection plan");
    let mut session = plan
        .session(&DetectionSamplingSessionOptions {
            random: RandomPolicy::Seeded(Seed::new(91)),
            ..Default::default()
        })
        .expect("create reusable detection session");
    let mut sink = NullSink::default();
    session
        .run(ShotCount::new(64), &mut sink)
        .expect("warm detection session");
    let measured = allocation_counter::measure(|| {
        for _ in 0..128 {
            session
                .run(ShotCount::new(64), &mut sink)
                .expect("reuse detection session");
        }
    });
    assert_eq!(
        measured.count_total, 0,
        "warmed detection session allocated while reusing bounded state: {measured:?}"
    );
    assert_eq!(sink.shots, 64 * 129);
}

/// The refusal of an exact `required`-byte requirement by a budget one byte smaller.
fn refusal(required: u128) -> stab_model::MemoryBudgetExceeded {
    let required = u64::try_from(required).expect("small requirement");
    stab_model::MemoryBudgetExceeded::new(
        stab_model::ResourceAmount::exact(required),
        stab_model::MemoryBudget::from_bytes(required - 1),
    )
}
