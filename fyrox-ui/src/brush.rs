//! Brush defines a way to fill an arbitrary surface. See [`Brush`] docs for more info and usage examples.

#![warn(missing_docs)]

use crate::core::{algebra::Vector2, color::Color, reflect::prelude::*, visitor::prelude::*};

/// Gradient point defines a point on a surface with a color.
#[derive(Clone, Debug, PartialEq, Reflect, Visit, Default)]
pub struct GradientPoint {
    /// A distance from an origin of the gradient.
    pub stop: f32,
    /// Color of the point.
    pub color: Color,
}

/// Brush defines a way to fill an arbitrary surface.
#[derive(Clone, Debug, PartialEq, Reflect, Visit)]
pub enum Brush {
    /// A brush, that fills a surface with a solid color.
    Solid(Color),
    /// A brush, that fills a surface with a linear gradient, which is defined by two points in local coordinates
    /// and a set of stop points. See [`GradientPoint`] for more info.
    LinearGradient {
        /// Beginning of the gradient in local coordinates.
        from: Vector2<f32>,
        /// End of the gradient in local coordinates.
        to: Vector2<f32>,
        /// Stops of the gradient.
        stops: Vec<GradientPoint>,
    },
    /// A brush, that fills a surface with a radial gradient, which is defined by a center point in local coordinates
    /// and a set of stop points. See [`GradientPoint`] for more info.
    RadialGradient {
        /// Center of the gradient in local coordinates.
        center: Vector2<f32>,
        /// Stops of the gradient.
        stops: Vec<GradientPoint>,
    },
}

impl Default for Brush {
    fn default() -> Self {
        Self::Solid(Color::WHITE)
    }
}
