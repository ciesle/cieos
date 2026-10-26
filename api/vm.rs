use crate::param::*;

use core::ops::Sub;
use core::ops::{Add, Deref, DerefMut};

pub trait RangeElement: Sized + Add<usize, Output = Self> + Clone + Copy {
    const STEP: usize;
    fn raw(self) -> usize;
    fn from_raw(addr: usize) -> Self;
    fn as_ptr(self) -> *const u8 {
        self.raw() as *const u8
    }
    fn as_mut_ptr(self) -> *mut u8 {
        self.raw() as *mut u8
    }
}

pub trait Addr: Copy + Eq + Ord + From<usize> + Into<usize> + Add<usize, Output = Self> {
    type Page: From<Self>;
    fn new(addr: usize) -> Self;
    fn page_down(self) -> Self {
        Self::new(self.into() & !(PG_SIZE - 1))
    }
    fn page_up(self) -> Self {
        Self::new((self.into() + PG_SIZE - 1) & !(PG_SIZE - 1))
    }
}
impl<A: Addr> RangeElement for A {
    const STEP: usize = 1;
    fn raw(self) -> usize {
        self.into()
    }
    fn from_raw(addr: usize) -> Self {
        A::new(addr)
    }
}

/// 仮想アドレス
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct VAddr(pub usize);
impl Addr for VAddr {
    type Page = Page;
    fn new(addr: usize) -> Self {
        Self(addr)
    }
}
impl From<VAddr> for usize {
    fn from(addr: VAddr) -> Self {
        addr.0
    }
}
impl From<usize> for VAddr {
    fn from(addr: usize) -> Self {
        Self(addr)
    }
}
impl Add<usize> for VAddr {
    type Output = Self;
    fn add(self, rhs: usize) -> Self {
        Self(self.0 + rhs)
    }
}
impl Sub<usize> for VAddr {
    type Output = Self;
    fn sub(self, rhs: usize) -> Self::Output {
        Self(self.0 - rhs)
    }
}
impl Sub<VAddr> for VAddr {
    type Output = usize;
    fn sub(self, rhs: VAddr) -> Self::Output {
        self.0 - rhs.0
    }
}
/// 物理アドレス
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PAddr(pub usize);
impl Addr for PAddr {
    type Page = Frame;
    fn new(addr: usize) -> Self {
        Self(addr)
    }
}
impl From<PAddr> for usize {
    fn from(addr: PAddr) -> Self {
        addr.0
    }
}
impl From<usize> for PAddr {
    fn from(addr: usize) -> Self {
        Self(addr)
    }
}
impl Add<usize> for PAddr {
    type Output = Self;
    fn add(self, rhs: usize) -> Self {
        Self(self.0 + rhs)
    }
}
impl Sub<usize> for PAddr {
    type Output = Self;
    fn sub(self, rhs: usize) -> Self::Output {
        Self(self.0 - rhs)
    }
}
impl Sub<PAddr> for PAddr {
    type Output = usize;
    fn sub(self, rhs: PAddr) -> Self::Output {
        self.0 - rhs.0
    }
}

/// 仮想アドレスページ
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Page(pub VAddr);
impl Page {
    pub const fn new(addr: VAddr) -> Self {
        Self(addr)
    }
    pub fn addr(self) -> VAddr {
        self.0
    }
    pub fn range(self) -> ARange<VAddr> {
        ARange::new(self.0, self.0 + PG_SIZE)
    }
}
impl From<VAddr> for Page {
    fn from(addr: VAddr) -> Self {
        assert!(usize::from(addr) % PG_SIZE == 0);
        Self(addr)
    }
}
impl From<Page> for VAddr {
    fn from(addr: Page) -> Self {
        addr.0
    }
}
impl Add<usize> for Page {
    type Output = Self;
    fn add(self, rhs: usize) -> Self {
        Self(self.0 + rhs * Self::STEP)
    }
}
impl Sub<usize> for Page {
    type Output = Self;
    fn sub(self, rhs: usize) -> Self::Output {
        Self(self.0 - rhs * Self::STEP)
    }
}
impl Sub<Page> for Page {
    type Output = usize;
    fn sub(self, rhs: Self) -> Self::Output {
        (self.0 - rhs.0) / Self::STEP
    }
}
impl RangeElement for Page {
    const STEP: usize = PG_SIZE;
    fn raw(self) -> usize {
        usize::from(VAddr::from(self))
    }
    fn from_raw(addr: usize) -> Self {
        Page::from(VAddr::from(addr))
    }
}
/// 物理アドレスフレーム
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Frame(pub PAddr);
impl Frame {
    pub const fn new(addr: PAddr) -> Self {
        Self(addr)
    }
    pub fn addr(self) -> PAddr {
        self.0
    }
    pub fn range(self) -> ARange<PAddr> {
        ARange::new(self.0, self.0 + PG_SIZE)
    }
}
impl From<PAddr> for Frame {
    fn from(addr: PAddr) -> Self {
        assert!(usize::from(addr) % PG_SIZE == 0);
        Self(addr)
    }
}
impl From<Frame> for PAddr {
    fn from(addr: Frame) -> Self {
        addr.0
    }
}
impl Add<usize> for Frame {
    type Output = Self;
    fn add(self, rhs: usize) -> Self {
        Self(self.0 + rhs * Self::STEP)
    }
}
impl Sub<usize> for Frame {
    type Output = Self;
    fn sub(self, rhs: usize) -> Self::Output {
        Self(self.0 - rhs * Self::STEP)
    }
}
impl Sub<Frame> for Frame {
    type Output = usize;
    fn sub(self, rhs: Self) -> Self::Output {
        (self.0 - rhs.0) / Self::STEP
    }
}
impl RangeElement for Frame {
    const STEP: usize = PG_SIZE;
    fn raw(self) -> usize {
        usize::from(PAddr::from(self))
    }
    fn from_raw(addr: usize) -> Self {
        Frame::from(PAddr::from(addr))
    }
}

#[derive(Clone, Copy)]
pub struct ARange<A: RangeElement> {
    pub start: A,
    pub end: A,
}
impl<A: RangeElement> ARange<A> {
    pub fn new(start: A, end: A) -> Self {
        Self {
            start: start,
            end: end,
        }
    }
    pub fn span(start: A, len: usize) -> Self {
        Self {
            start,
            end: A::from_raw(start.raw().wrapping_add(len.wrapping_mul(A::STEP))),
        }
    }
    pub fn len(self) -> usize {
        (self.end.raw().wrapping_sub(self.start.raw())) / A::STEP
    }
    pub fn step_by(self, step: usize) -> impl Iterator<Item = A> {
        self.into_iter().step_by(step)
    }
}
impl ARange<VAddr> {
    pub fn from_ref<T: Sized>(value: &T) -> Self {
        Self::span(VAddr(value as *const T as usize), core::mem::size_of::<T>())
    }
    pub fn from_mut_ref<T: Sized>(value: &mut T) -> Self {
        Self::span(VAddr(value as *mut T as usize), core::mem::size_of::<T>())
    }
}
/// アドレス範囲の参照を返す。アドレスが現在のページテーブルでマップされていることが前提
impl<A: Addr> Deref for ARange<A> {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        unsafe { core::slice::from_raw_parts(self.start.as_ptr(), self.len()) }
    }
}
/// DerefMutを実装するには、対象のメモリ範囲が排他的にアクセスできることを保証できる必要がある
/// 今回は多分大丈夫だろうということで実装しているが、本来は厳密な検証が必要
impl<A: Addr> DerefMut for ARange<A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { core::slice::from_raw_parts_mut(self.start.as_mut_ptr(), self.len()) }
    }
}
impl<A: Addr> From<&[u8]> for ARange<A> {
    fn from(addr: &[u8]) -> Self {
        Self::new(
            A::new(addr.as_ptr() as usize),
            A::new(addr.as_ptr() as usize + addr.len()),
        )
    }
}
impl<A: RangeElement> IntoIterator for ARange<A> {
    type Item = A;
    type IntoIter = core::iter::Map<core::iter::StepBy<core::ops::Range<usize>>, fn(usize) -> A>;
    fn into_iter(self) -> Self::IntoIter {
        let start: usize = self.start.raw();
        let end: usize = self.end.raw();
        (start..end).step_by(A::STEP).map(A::from_raw)
    }
}
