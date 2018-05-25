// "ami" crate - Licensed under the MIT LICENSE
//  * Copyright (c) 2017  Douglas P. Lau
//  * Copyright (c) 2017-2018  Jeron A. Lau <jeron.lau@plopgrizzly.com>

use std::{ fmt, ops };

use Vec3;
use BCube;

/// Bounding box
#[derive(Clone, Copy)]
pub struct BBox {
	pub(crate) min: Vec3,
	pub(crate) max: Vec3,
}

impl fmt::Debug for BBox {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{:?} → {:?}", self.min, self.max)
	}
}

impl ops::Sub<Vec3> for BBox {
	type Output = BBox;

	fn sub(self, other: Vec3) -> Self::Output {
		BBox::new(self.min - other, self.max - other)
	}
}

impl ops::Add<Vec3> for BBox {
	type Output = BBox;

	fn add(self, other: Vec3) -> Self::Output {
		BBox::new(self.min + other, self.max + other)
	}
}

impl BBox {
	/// Create an new `BBox` at position `p`.
	pub fn new(min: Vec3, max: Vec3) -> BBox {
		assert!(min.x <= max.x);
		assert!(min.y <= max.y);
		assert!(min.z <= max.z);

		BBox { min, max }
	}

	/// Check if `BBox` collides with `other` `BBox`.
	pub fn collide(&self, other: BBox) -> bool {
		   other.max.x >= self.min.x
		&& self.max.x >= other.min.x
		&& other.max.y >= self.min.y
		&& self.max.y >= other.min.y
		&& other.max.z >= self.min.z
		&& self.max.z >= other.min.z
	}

	/// Check if `BBox` collides with `BCube`.
	pub fn collide_bcube(&self, bcube: BCube) -> bool {
		let (max, min) = bcube.to_point_pair();
		self.collide(BBox::new(min, max))
	}

	/// Check if `BBox` collides with point `p`.
	pub fn collide_vec3(&self, p: Vec3) -> bool {
		(p.x >= self.min.x) &&
		(p.x <= self.max.x) &&
		(p.y >= self.min.y) &&
		(p.y <= self.max.y) &&
		(p.z >= self.min.z) &&
		(p.z <= self.max.z)
	}

	/// Get all 8 points of the `BBox`.
	pub fn all_points(&self) -> [Vec3; 8] {
		[
			Vec3::new(self.min.x, self.min.y, self.min.z),
			Vec3::new(self.min.x, self.min.y, self.max.z),
			Vec3::new(self.min.x, self.max.y, self.min.z),
			Vec3::new(self.min.x, self.max.y, self.max.z),
			Vec3::new(self.max.x, self.min.y, self.min.z),
			Vec3::new(self.max.x, self.min.y, self.max.z),
			Vec3::new(self.max.x, self.max.y, self.min.z),
			Vec3::new(self.max.x, self.max.y, self.max.z),
		]
	}

	/// Get the center of the `BBox`.
	pub fn center(&self) -> Vec3 {
		Vec3::new(
			(self.min.x + self.max.x) / 2.0,
			(self.min.y + self.max.y) / 2.0,
			(self.min.z + self.max.z) / 2.0,
		)
	}
}