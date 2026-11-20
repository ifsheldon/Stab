#![allow(
    clippy::expect_used,
    reason = "the resource-boundary test uses exact compact fixture assertions"
)]

use std::collections::BTreeMap;

use stab_model::{
    DemDetectorId, DetectorErrorModel, MemoryBudget, ResourceKind, ResourceOperation,
};

#[test]
fn dem_coordinate_all_map_is_admitted_against_the_memory_budget_but_selected_queries_are_not() {
    let dem = DetectorErrorModel::from_dem_str(
        "repeat 1000000000000 {\n\
             detector(1, 2) D0\n\
             shift_detectors(3, 4) 1\n\
         }\n",
    )
    .expect("parse huge coordinate DEM");

    let error = dem
        .detector_coordinates()
        .expect_err("reject an all-detector map beyond the default budget before allocating");
    assert!(error.to_string().contains("memory budget"), "{error}");
    let resource = error
        .resource_limit_error()
        .expect("typed resource refusal");
    assert_eq!(resource.resource(), ResourceKind::MaterializedBytes);
    assert_eq!(
        resource.operation(),
        ResourceOperation::DetectorErrorModelDetectorCoordinates
    );

    let small = DetectorErrorModel::from_dem_str(
        "repeat 3 {\n\
             detector(1, 2) D0\n\
             shift_detectors(3, 4) 1\n\
         }\n",
    )
    .expect("parse small coordinate DEM");
    // Three entries of 96 bytes plus two coordinate scalars each fit exactly 336 bytes.
    let exact = MemoryBudget::from_bytes(3 * (96 + 2 * 8));
    assert_eq!(
        small
            .detector_coordinates_with_memory_budget(exact)
            .expect("exact budget admits the map")
            .len(),
        3
    );
    let excess = MemoryBudget::from_bytes(exact.bytes() - 1);
    let measured = allocation_counter::measure(|| {
        std::hint::black_box(
            small
                .detector_coordinates_with_memory_budget(excess)
                .expect_err("one byte below the requirement is refused"),
        );
    });
    assert!(measured.bytes_max < 1024, "{measured:?}");

    let detector_0 = DemDetectorId::try_new(0).expect("D0");
    let detector_1 = DemDetectorId::try_new(1).expect("D1");
    assert_eq!(
        dem.detector_coordinates_for([detector_0, detector_1])
            .expect("selected huge-repeat coordinates"),
        BTreeMap::from([(detector_0, vec![1.0, 2.0]), (detector_1, vec![4.0, 6.0]),])
    );
    assert_eq!(
        dem.coordinates_of_detector(detector_1)
            .expect("single detector coordinates"),
        vec![4.0, 6.0]
    );
}
