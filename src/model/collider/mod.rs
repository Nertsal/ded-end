#![allow(dead_code)]

mod shape;

pub use self::shape::*;

use super::*;

use geng_utils::conversions::{RealConversions, Vec2RealConversions};

#[derive(Debug, Clone, Copy)]
pub struct Collision {
    pub point: vec2<FCoord>,
    /// Normal vector pointing away from the body.
    pub normal: vec2<FCoord>,
    pub penetration: FCoord,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Transform {
    pub translation: vec2<FCoord>,
    pub rotation: Angle<FCoord>,
    pub scale: FCoord,
}

impl Transform {
    pub fn identity() -> Self {
        Self {
            translation: vec2::ZERO,
            rotation: Angle::ZERO,
            scale: FCoord::ONE,
        }
    }

    pub fn scale(scale: impl Float) -> Self {
        Self {
            scale: scale.as_r32(),
            ..Self::identity()
        }
    }

    pub fn lerp(&self, target: &Self, t: FTime) -> Self {
        Self {
            translation: self.translation + (target.translation - self.translation) * t,
            rotation: self.rotation + self.rotation.angle_to(target.rotation) * t,
            scale: self.scale + (target.scale - self.scale) * t,
        }
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::identity()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Collider {
    pub position: vec2<FCoord>,
    #[serde(default = "default_rotation")]
    pub rotation: Angle<FCoord>,
    pub shape: Shape,
}

fn default_rotation() -> Angle<FCoord> {
    Angle::ZERO
}

impl Collider {
    pub fn circle(center: vec2<FCoord>, radius: FCoord) -> Self {
        Self::new(center, Shape::circle(radius))
    }

    pub fn new(position: vec2<FCoord>, shape: Shape) -> Self {
        Self {
            position,
            rotation: Angle::ZERO,
            shape,
        }
    }

    pub fn aabb(aabb: Aabb2<FCoord>) -> Self {
        Self::new(aabb.center(), Shape::rectangle(aabb.size()))
    }

    pub fn transform_mat(&self) -> mat3<FCoord> {
        mat3::translate(self.position) * mat3::rotate(self.rotation)
    }

    pub fn compute_aabb(&self) -> Aabb2<FCoord> {
        let (iso, shape) = self.to_parry();
        let parry2d::bounding_volume::Aabb { mins, maxs } = shape.compute_aabb(&iso);
        Aabb2 {
            min: vec2(mins.x, mins.y).as_r32(),
            max: vec2(maxs.x, maxs.y).as_r32(),
        }
    }

    fn get_iso(&self) -> parry2d::math::Isometry<f32> {
        let vec2(x, y) = self.position.as_f32();
        let angle = self.rotation.as_radians().as_f32();
        parry2d::math::Isometry::new(parry2d::na::Vector2::new(x, y), angle)
    }

    fn to_parry(&self) -> (parry2d::math::Isometry<f32>, Box<dyn parry2d::shape::Shape>) {
        (self.get_iso(), self.shape.to_parry())
    }

    /// Check whether the collider contains the point.
    pub fn contains(&self, point: vec2<FCoord>) -> bool {
        // TODO: better
        self.check(&Collider {
            position: point,
            rotation: Angle::ZERO,
            shape: Shape::Circle { radius: r32(0.01) },
        })
    }

    /// Check whether two colliders are intersecting.
    pub fn check(&self, other: &Self) -> bool {
        let delta = (other.position - self.position).as_f32();

        let self_angle = self.rotation.as_radians().as_f32();
        let self_iso = parry2d::math::Isometry::rotation(self_angle);
        let self_shape = self.shape.to_parry();

        let other_angle = other.rotation.as_radians().as_f32();
        let other_iso =
            parry2d::math::Isometry::new(parry2d::na::Vector2::new(delta.x, delta.y), other_angle);
        let other_shape = other.shape.to_parry();

        parry2d::query::intersection_test(&self_iso, &*self_shape, &other_iso, &*other_shape)
            .unwrap()
    }

    /// Return the collision info if the two colliders are intersecting.
    pub fn collide(&self, other: &Self) -> Option<Collision> {
        let delta = (other.position - self.position).as_f32();

        let self_angle = self.rotation.as_radians().as_f32();
        let self_iso = parry2d::math::Isometry::rotation(self_angle);
        let self_shape = self.shape.to_parry();

        let other_angle = other.rotation.as_radians().as_f32();
        let other_iso =
            parry2d::math::Isometry::new(parry2d::na::Vector2::new(delta.x, delta.y), other_angle);
        let other_shape = other.shape.to_parry();

        let prediction = 0.0;
        parry2d::query::contact(
            &self_iso,
            &*self_shape,
            &other_iso,
            &*other_shape,
            prediction,
        )
        .unwrap()
        .map(|contact| {
            let normal = contact.normal1.into_inner();
            let point = contact.point1;
            Collision {
                point: vec2(point.x, point.y).map(FCoord::new),
                normal: vec2(normal.x, normal.y).map(FCoord::new),
                penetration: FCoord::new(-contact.dist),
            }
        })
    }
}
