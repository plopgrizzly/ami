// "ami" crate - Licensed under the MIT LICENSE
//  * Copyright (c) 2017  Douglas P. Lau
//  * Copyright (c) 2017-2018  Jeron A. Lau <jeron.lau@plopgrizzly.com>

use std::fmt;
// use std::cmp::Ordering;

use Vec3;
use BCube;
use BBox;
use Collider;
// use Frustum;

/// An octree is a DAG that can quickly search for points in 3D space.
///
/// The bounding box of the root node contains all points in the octree.
/// If a point outside the bounding box is added, a new root node is created
/// which contains the old root as one of its octants.  This process is repeated
/// until the point is contained.
///
/// The nodes are stored in a vector, and are indexed using a 32-bit node ID.
/// This saves memory over using pointers on 64-bit systems.  Node ID 1 is the
/// first node in the vector.
pub struct Octree<T: Collider> {
	colliders: Vec<T>,
	collider_garbage: Vec<Id>,
	nodes: Vec<Node>,
	garbage: Vec<Id>,
	bcube: BCube,
	root: Id,
	n_colliders: u32,
}

const LINK: usize = 15;			// link to coincident leaf nodes
const LEAF: u32 = 0xFF_FF_FF_FF;	// max u32 value (invalid handle)

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct Id(u32);

impl Id {
	/// Get an `Id` that represents nothing.
	fn none() -> Self {
		Id(0)
	}

	/// Does this `Id` represent nothing?
	fn is_none(&self) -> bool {
		self.0 == 0
	}

	/// Does this `Id` represent something?
	fn is_some(&self) -> bool {
		!self.is_none()
	}
}

impl Into<Id> for usize {
	fn into(self) -> Id {
		Id(self as u32 + 1)
	}
}

impl Into<usize> for Id {
	fn into(self) -> usize {
		(self.0 - 1) as usize
	}
}

/// A node is either a branch or a leaf.
///
/// A branch can have up to 8 child nodes (each octant adjacent to the center)
/// and 7 objects, plus an optional link to a leaf.
///
/// A leaf can store up to 14 points; the first child must contain a LEAF
/// sentinel value, and the last may link to another leaf node.
///
/// Each node has an implicit bounding box determined by its position in the
/// octree.  The bounding box contains all descendant nodes.
struct Node {
	/// child node handles
	child: [Id; 16],
}

impl fmt::Debug for Node {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		if self.is_leaf() {
			try!(write!(f, "leaf"));
//			try!(write!(f, "leaf: {:?}", self.leaf_children()));
			let l = self.link();
			if l.is_some() {
				try!(write!(f, " link: {:?}", l));
			}
			Ok(())
		} else {
			write!(f, "branch: {:?}", self.child)
		}
	}
}

impl Node {
	/// Create a new leaf node
	fn new_leaf() -> Node {
		Node {
			// no elements, no linking
			child: [
				Id(LEAF), Id::none(), Id::none(), Id::none(),
				Id::none(), Id::none(), Id::none(), Id::none(),
				Id::none(), Id::none(), Id::none(), Id::none(),
				Id::none(), Id::none(), Id::none(), Id::none()
			],
		}
	}

	/// Create a new branch node
	fn new_branch() -> Node {
		Node {
			child: [Id::none(); 16],
		}
	}

	/// Test if a node is a leaf
	fn is_leaf(&self) -> bool {
		self.child[0] == Id(LEAF)
	}

	/// Test if a node is a branch
	fn is_branch(&self) -> bool {
		!self.is_leaf()
	}

	/// Get link to next link node ID
	fn link(&self) -> Option<usize> {
		// Can be a branch or a leaf.
		if self.child[LINK].is_none() {
			// No link found - shouldn't happen.
			None
		} else {
			// Convert link Id to usize
			Some(self.child[LINK].into())
		}
	}

	/// None has no child branches and no collider Ids
	fn is_empty(&self) -> bool {
		assert!(self.is_branch());
		// First 8 are branches.
		for i in &self.child[..=14] { // skip link
			if i.is_some() {
				return false // isn't empty
			}
		}

		true // is empty
	}

	/// Find the only full ch. branch, if there is only one, None otherwise.
	fn branch_is_one(&self) -> Option<usize> {
		assert!(self.is_branch());
		// First 8 are branches.
		let mut found = Id::none();

		for i in &self.child[..8] { // First 8 are branches
			if i.is_some() {
				if found.is_some() { // 2, not 1
					return None;
				} else {
					found = *i;
				}
			}
		}
		for i in &self.child[8..=14] { // Skip link are collider Ids
			if i.is_some() {
				return None // isn't empty
			}
		}

		if found.is_some() { // 1
			Some(found.into())
		} else { // None
			None
		}
	}

	/// Find the first open child slot in a branch, None if full.
	fn branch_open_slot(&self) -> Option<usize> {
		assert!(self.is_branch());
		// Skip 0-7 as that is descending the octree, and skip 15 (link)
		let slot = self.child[8..=14].iter().position(|v| v.is_none());
		if let Some(s) = slot {
			return Some(s); // same as slot
		}
		None
	}

	/// Add a collider to a branch node.
	fn branch_add_collider(&mut self, id: Id) -> Option<()> {
		assert!(self.is_branch());
		let s = self.branch_open_slot()?;
		self.child[s] = id;
		// Successfully added it.
		Some(())
	}

	/// Remove a collider from a branch node.
	fn branch_remove_collider(&mut self, id: Id) -> Option<()> {
		assert!(self.is_branch());
		// Look for collider in this branch.
		for i in 8..=14 {
			// Delete it when found, and return successfully.
			if self.child[i] == id {
				self.child[i] = Id::none();
				return Some(());
			}
		}
		// Not here, look in links next.
		return None;
	}

	/// Remove a collider from a leaf node.
	fn leaf_remove_collider(&mut self, id: Id) -> Option<()> {
		assert!(self.is_leaf());
		// Look for collider in this branch.
		for i in 1..=14 {
			// Delete it when found, and return successfully.
			if self.child[i] == id {
				self.child[i] = Id::none();
				return Some(());
			}
		}
		// Not here, look in links next.
		return None;
	}

	/// Remove a collider from a node.
	fn remove_collider(&mut self, id: Id) -> Option<()> {
		if self.is_branch() {
			self.branch_remove_collider(id)
		} else {
			self.leaf_remove_collider(id)
		}
	}

	/// Determine which child for a branch point
	fn which_child(c: Vec3, p: Vec3) -> usize {
		match (p.x < c.x, p.y < c.y, p.z < c.z) {
			(true,  true,  true)  => 0,
			(true,  true,  false) => 1,
			(true,  false, true)  => 2,
			(true,  false, false) => 3,
			(false, true,  true)  => 4,
			(false, true,  false) => 5,
			(false, false, true)  => 6,
			(false, false, false) => 7,
		}
	}

	/// Determine which child for a branch bbox, if there is one it fully
	/// fits into.
	fn which_child_bbox(c: Vec3, p: BBox) -> Option<usize> {
		let min = Self::which_child(c, p.min);
		let max = Self::which_child(c, p.max);

		if min == max {
			Some(min)
		} else {
			None
		}
	}

	/// Calculate the center of a child node
	fn child_center(ch: usize, c: Vec3, h: f32) -> Vec3 {
		let h = if h < 0.000001 { 1.0 } else { h };

		match ch {
			0 => Vec3::new(c.x - h, c.y - h, c.z - h),
			1 => Vec3::new(c.x - h, c.y - h, c.z + h),
			2 => Vec3::new(c.x - h, c.y + h, c.z - h),
			3 => Vec3::new(c.x - h, c.y + h, c.z + h),
			4 => Vec3::new(c.x + h, c.y - h, c.z - h),
			5 => Vec3::new(c.x + h, c.y - h, c.z + h),
			6 => Vec3::new(c.x + h, c.y + h, c.z - h),
			7 => Vec3::new(c.x + h, c.y + h, c.z + h),
			a => panic!("ch must be 0-7, not {}", a),
		}
	}

	/// Calculate the bounding box of a child node
	fn child_bcube(ch: usize, bcube: BCube) -> BCube {
		assert!(bcube.half_len > 0.0);
		let half_len = bcube.half_len / 2.0;
		let center = Node::child_center(ch, bcube.center, half_len);
		BCube { center: center, half_len: half_len }
	}

/*	/// Get an array containing the leaf children
	fn branch_children(&self) -> [u32; 7] {
		assert!(self.is_leaf());

		[self.child[1], self.child[2], self.child[3], self.child[4],
			self.child[5], self.child[6]]
	}*/
}

impl<T> Octree<T> where T: Collider {
	/// Create a new octree
	pub fn new() -> Octree<T> {
		Octree {
			colliders: vec![],
			collider_garbage: vec![],
			nodes: vec![],
			garbage: vec![],
			bcube: BCube::empty(),
			root: Id::none(),
			n_colliders: 0,
		}
	}

	/// Add a point in the octree
	pub fn add(&mut self, point: T) -> Id {
		// Add to colliders and get the id.
		let id = if let Some(id) = self.collider_garbage.pop() {
			unsafe {
				::std::ptr::copy_nonoverlapping(&point,
					&mut self.colliders[{ let id: usize = id.into(); id }], 1);
			}
			::std::mem::forget(point); // don't drop it, it's moved!
			id
		} else {
			self.colliders.push(point);
			Id(self.colliders.len() as u32)
		};

		// Find position in octree for this new collider.
		match self.n_colliders {
			0 => self.add_0(id),
			_ => self.add_n(id),
		}

		// Increment number of colliders, and return id
		self.n_colliders += 1;
		id
	}

	/// Add a point when empty
	fn add_0(&mut self, id: Id) {
		// Number of colliders must be 0
		assert!(self.n_colliders == 0);

		// Clear the octree
		self.nodes.clear();
		self.garbage.clear();

		// Make the root bcube contain the bbox of this first point.
		self.bcube = self[id].bbox().into();

		// Build the branch and add a collider.
		let i = self.new_branch();
		self.nodes[{ let i: usize = i.into(); i }].branch_add_collider(id).unwrap();

		// Set this branch as the root node.
		self.root = i;
	}

	/// Add a point when not empty
	fn add_n(&mut self, id: Id) {
		// Must have colliders already in the octree.
		assert!(self.n_colliders > 0);
		// Get BBox
		let bbox = self[id].bbox();
		let bcube = self.bcube;
		let root = self.root;

		// While the bbox isn't within the root bcube, expand root bcube
		while !bbox.collide_bcube(bcube) {
			self.grow_root(bbox);
		}

		// Add id inside the root bcube.
		self.add_inside(id, root, bcube);
	}

	/// Grow the root node
	fn grow_root(&mut self, bbox: BBox) {
		// BBox can't collide with bcube when this function is called.
		assert!(!bbox.collide_bcube(self.bcube));
		assert!(self.nodes[{ let a: usize = self.root.into(); a }].is_branch());

		// Get the old bcube center, to see which octant it goes in.
		let center = self.bcube.center;

		// Extend bcube to attempt to accomodate for bbox.
		// This function is limited to growing twice in size.
		self.bcube.extend(bbox);

		// Create new container branch for old root branch.
		let ch = Node::which_child(self.bcube.center, center);
		let id = self.new_branch();
		self.nodes[{ let a: usize = id.into(); a }].child[ch] = self.root;
		self.root = id;
	}

	/// Add a point within the bounds
	fn add_inside(&mut self, id: Id, node_id: Id, bcube: BCube) {
		// Calculate bbox for this id.
		let bbox = self[id].bbox();
		// Convert node_id to usize for indexing.
		let node_id: usize = node_id.into();

		// BBox must collide with bcube when this function is called
		assert!(bbox.collide_bcube(bcube));
		// Must be a branch
		assert!(self.nodes[node_id].is_branch());

		// Attempt to add at root first.  Test is full
		if self.nodes[node_id].branch_add_collider(id).is_none() {
			// Attempt to push relative root colliders down the tree
			for i in 8..=14 {
				let collider = self.nodes[node_id].child[i];
				if self.add_down(collider, node_id, bcube) {
					// If it successfully pushed it the
					// collider down the octree, remove it
					// from it's old location.
					self.nodes[node_id].child[i]
						= Id::none();
				}
			}

			// Attempt to push this collider (id) down the tree
			if self.add_down(id, node_id, bcube) {
				return;
			}

			// Try again, this time link if failed.
			if self.nodes[node_id].branch_add_collider(id)
				.is_none() // Is full, still!
			{
				let link_id = self.new_leaf();
				self.nodes[node_id].child[LINK]
					= link_id;
			}
		}
	}

	/// Move a collider down the tree, return true if it worked.
	fn add_down(&mut self, id: Id, node_id: usize, bcube: BCube) -> bool {
		// Calculate bbox for this id.
		let bbox = self[id].bbox();

		// can be put on a lower level.
		if let Some(ch) = Node::which_child_bbox(bcube.center, bbox) {
			let j = self.nodes[node_id].child[ch];
			let bc = Node::child_bcube(ch, bcube);

			if j.is_some() {
				// already a branch here, add collider to it.
				self.add_inside(id, j, bc);
			} else {
				// make a branch
				let k = self.new_branch();
				// set branch as the correct child
				self.nodes[node_id].child[ch] = k;
				// Add the collider
				self.nodes[{ let k: usize = k.into(); k }]
					.branch_add_collider(id)
					.unwrap(); // shouldn't fail.
			}
			true
		} else {
			false
		}
	}

	/// Add a new node
	fn new_node(&mut self, n: Node) -> Id {
		if let Some(i) = self.garbage.pop() {
			let k: usize = i.into();
			self.nodes[k] = n;
			k.into()
		} else {
			self.nodes.push(n);
			Id(self.nodes.len() as u32)
		}
	}

	/// Add a new leaf node
	fn new_leaf(&mut self) -> Id {
		self.new_node(Node::new_leaf())
	}

	/// Add a new branch node
	fn new_branch(&mut self) -> Id {
		self.new_node(Node::new_branch())
	}

	/// Remove a point from the octree
	pub fn remove(&mut self, id: Id) -> T {
		// Must have colliders already in the octree.
		assert!(self.n_colliders > 0);
		// 
		let bcube = self.bcube;
		let root = self.root;
		// Find and remove the collider Id from the octree.
		// Should always be None TODO: maybe if len() is 1, octree should be emptied and actually returns None
		assert_eq!(self.remove_inside(id, root, bcube), None);
		// For indexing.
		let root: usize = self.root.into();
		// Id is garbage now.
		self.collider_garbage.push(id);
		// Shrink root if: 1 branch, no nodes
		if let Some(ch) = self.nodes[root].branch_is_one() {
			// Add root to garbage.
			self.garbage.push(self.root);
			// Set new root
			self.root = self.nodes[root].child[ch];
		}
		// Decrement number of colliders
		self.n_colliders -= 1;

		// Return the memory by copy.
		unsafe {
			let mut ret = ::std::mem::uninitialized();
			::std::ptr::copy_nonoverlapping(
				&self.colliders[{ let id: usize = id.into(); id }], &mut ret, 1);
			ret
		}
	}

	/// Remove an Id from the octree.
	fn remove_inside(&mut self, id: Id, node_id: Id, bcube: BCube)
		-> Option<Id>
	{
		// Calculate bbox for this id.
		let bbox = self[id].bbox();
		// Get node_id as usize
		let node_id: usize = node_id.into();

		// BBox must collide with bcube when this function is called
		assert!(bbox.collide_bcube(bcube));
		// Must be a branch
		assert!(self.nodes[node_id].is_branch());

		// Could be found on a lower level.
		if let Some(ch) = Node::which_child_bbox(bcube.center, bbox) {
			let j = self.nodes[node_id].child[ch];

			if j.is_some() {
				// Yes, there is a branch here, where the Id is!
				// Remove it from inside this branch.
				if let Some(rm)
					= self.remove_inside(id, j, bcube)
				{ // Remove empty branch
					// Child branch should be the one
					// removed
					assert_eq!(j, rm);
					// Add to garbage.
					self.garbage.push(rm);
					// Remove child branch.
					self.nodes[node_id].child[ch] =
						Id::none();
				}
				None // nothing to be removed.
			} else {
				// No, we don't have to descend - it's here!
				self.remove_from_branch(id, node_id)
			}
		} else {
			// No, we can't descend - it's here!
			self.remove_from_branch(id, node_id)
		}
	}

	/// Remove from branch, including any links that may exist.
	fn remove_from_branch(&mut self, id: Id, node_id: usize) -> Option<Id> {
		// Remove the collider
		if self.nodes[node_id].remove_collider(id)
			.is_some() // Found and removed
		{
			// If the node is empty, mark for removal.
			if self.nodes[node_id].is_empty() {
				return Some(node_id.into());
			} else {
				return None;
			}
		}

		// Couldn't Find it: Search Link Node
		let node_id = self.nodes[node_id].link()
			.unwrap(); // Shouldn't fail if not found yet.
		let rm = self.remove_from_branch(id, node_id);

		// If link leaf is now empty, remove.
		if let Some(rm) = rm {
			// Returned location should match LINK node location.
			assert_eq!(rm, self.nodes[node_id].child[LINK]);
			// Add to garbage.
			self.garbage.push(rm);
			// Remove Link.
			self.nodes[node_id].child[LINK] = Id::none();
		}

		None // Don't remove this node
	}
}

impl<T> ::std::ops::Index<Id> for Octree<T> where T: Collider {
	type Output = T;

	fn index<'a>(&'a self, index: Id) -> &'a T {
		let index: usize = index.into();
		&self.colliders[index]
	}
}

impl<T> ::std::ops::IndexMut<Id> for Octree<T> where T: Collider {
	fn index_mut<'a>(&'a mut self, index: Id) -> &'a mut T {
		let index: usize = index.into();
		&mut self.colliders[index]
	}
}
