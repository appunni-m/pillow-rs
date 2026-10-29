// NO_PILLOW_SUPPORT: This native core mode supplies dimensions only; noise output is L.
pub(crate) use crate::compute::pool_cpu::ops::effects::helpers::noise_vector::{
    prepare, execute_neon as execute,
};
