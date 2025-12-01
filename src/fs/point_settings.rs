//! Re-exports PointSettings from the shader in a format that is amenable to being put in JSON.

macro_rules! point_settings {
    (pub struct $name:ident { $(
        $from:ident -> $to:ident ,
    )* }) => {
        use crate::shaders::compute_shader;
        #[derive(Debug, Clone, facet::Facet)]
        pub struct $name { $(
            pub $to: f32,
        )* }

        impl $name {
            pub fn random_base() -> Self {
                let mut rng = rand::rng();
                Self { $(
                    $to: super::sample_base_setting(&mut rng),
                )* }
            }
        }

        impl From<compute_shader::PointSettings> for $name {
            fn from(s: compute_shader::PointSettings) -> Self {
                Self { $(
                    $to: s.$from,
                )* }
            }
        }

        impl From<$name> for compute_shader::PointSettings {
            fn from(s: $name) -> Self {
                Self { $(
                    $from: s.$to,
                )* }
            }
        }

        impl std::ops::Add<$name> for $name {
            type Output = $name;
            fn add(self, rhs: $name) -> Self::Output {
                Self::Output { $(
                    $to: self.$to + rhs.$to,
                )* }
            }
        }

        impl std::ops::Mul<f32> for $name {
            type Output = $name;
            fn mul(self, rhs: f32) -> Self::Output {
                Self::Output { $(
                    $to: self.$to * rhs,
                )* }
            }
        }
    }
}

point_settings! {
    pub struct PointSettings {
        sd_base                -> sd0,
        sd_amplitude           -> sda,
        sd_exponent            -> sde,
        sa_base                -> sa0,
        sa_amplitude           -> saa,
        sa_exponent            -> sae,
        ra_base                -> ra0,
        ra_amplitude           -> raa,
        ra_exponent            -> rae,
        md_base                -> md0,
        md_amplitude           -> mda,
        md_exponent            -> mde,
        default_scaling_factor -> dsf,
        sensor_bias_1          -> sb1,
        sensor_bias_2          -> sb2,
    }
}
