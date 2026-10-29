//! Immutable backend and target selection carried through pipeline execution.

use super::Backend;
use std::sync::OnceLock;

/// Architecture selected for one backend execution path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Architecture {
    /// Serial scalar implementation and the guaranteed CPU fallback.
    Scalar,
    /// x86 FMA capability used by a serial CPU convolution kernel.
    Fma,
    /// Portable vector implementation without a dedicated host kernel.
    PortableSimd,
    /// AArch64 NEON implementation.
    Neon,
    /// x86 SSSE3 implementation.
    Ssse3,
    /// x86 AVX2 implementation.
    Avx2,
    /// The current WebGPU execution path.
    Wgpu,
    /// Reserved for a future CUDA execution path.
    Cuda,
    /// Reserved for a future TileLang execution path.
    TileLang,
}

impl Architecture {
    pub(crate) const fn supports_simd(self) -> bool {
        matches!(
            self,
            Self::PortableSimd | Self::Neon | Self::Ssse3 | Self::Avx2
        )
    }

    pub(crate) const fn supports_cpu(self) -> bool {
        matches!(self, Self::Scalar | Self::Fma)
    }

    pub(crate) fn for_backend(backend: Backend) -> Self {
        match backend {
            Backend::Cpu | Backend::ParallelCpu => Self::host_cpu(),
            Backend::Gpu => Self::Wgpu,
            Backend::Simd => Self::host_simd(),
        }
    }

    fn host_cpu() -> Self {
        static HOST_CPU: OnceLock<Architecture> = OnceLock::new();
        *HOST_CPU.get_or_init(|| {
            #[cfg(target_arch = "x86_64")]
            if std::is_x86_feature_detected!("fma") {
                return Self::Fma;
            }
            Self::Scalar
        })
    }

    fn host_simd() -> Self {
        static HOST_SIMD: OnceLock<Architecture> = OnceLock::new();
        *HOST_SIMD.get_or_init(Self::detect_host_simd)
    }

    fn detect_host_simd() -> Self {
        #[cfg(target_arch = "aarch64")]
        {
            return Self::Neon;
        }

        #[cfg(target_arch = "x86_64")]
        {
            if std::is_x86_feature_detected!("avx2") {
                return Self::Avx2;
            }
            if std::is_x86_feature_detected!("ssse3") {
                return Self::Ssse3;
            }
        }

        Self::PortableSimd
    }
}

/// The selected backend and architecture for a pipeline execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ExecutionContext {
    backend: Backend,
    architecture: Architecture,
}

impl ExecutionContext {
    /// Select a backend and detect its architecture once at the routing boundary.
    pub(crate) fn for_backend(backend: Backend) -> Self {
        Self {
            backend,
            architecture: Architecture::for_backend(backend),
        }
    }

    /// Build an explicit context for a planned family or segment.
    pub(crate) const fn new(backend: Backend, architecture: Architecture) -> Self {
        Self {
            backend,
            architecture,
        }
    }

    /// Return the backend selected for this pipeline segment.
    pub(crate) const fn backend(self) -> Backend {
        self.backend
    }

    /// Return the target selected for this pipeline segment.
    pub(crate) const fn architecture(self) -> Architecture {
        self.architecture
    }

    /// Whether the selected CPU implementation uses hardware FMA kernels.
    pub(crate) const fn uses_cpu_fma(self) -> bool {
        matches!(self.backend, Backend::Cpu | Backend::ParallelCpu)
            && matches!(self.architecture, Architecture::Fma)
    }

    /// Select the target associated with a backend when routing a segment.
    pub(crate) fn routed_to(self, backend: Backend) -> Self {
        Self::new(backend, Architecture::for_backend(backend))
    }
}
