#![allow(
    clippy::expect_used,
    reason = "the resource-boundary test uses exact compact fixture assertions"
)]

use std::collections::BTreeMap;

use stab_model::{DemDetectorId, DetectorErrorModel};

#[test]
fn pf4_dem_coordinates_reject_huge_all_map_but_allow_selected_queries() {
    let dem = DetectorErrorModel::from_dem_str(
        "repeat 1000001 {\n\
             detector(1, 2) D0\n\
             shift_detectors(3, 4) 1\n\
         }\n",
    )
    .expect("parse huge coordinate DEM");

    let error = dem
        .detector_coordinates()
        .expect_err("reject huge all-detector coordinate map");
    assert!(
        error
            .to_string()
            .contains("detector_coordinates currently supports at most 1000000"),
        "{error}"
    );

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
