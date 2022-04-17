#[macro_export]
macro_rules! self_action {
    ($self:ident, $name:literal, $method:ident) => {
        {
            let this = &$self;
            let action = gio::SimpleAction::new($name, None);
            action.connect_activate(clone!(@weak this => move |_,_| this.$method()));
            $self.add_action(&action);
            action
        }
    }
}

#[macro_export]
macro_rules! view {
    ($name:ident = ($obj_e:expr) {$($tt:tt)+} $($ctt:tt)*) => {
        let $name = $obj_e;
        view!(@expand-build $name $($tt)+);
        view!($($ctt)*);
    };
    ($name:ident = $obj_ty:path {$($tt:tt)+} $($ctt:tt)*) => {
        let _obj: $obj_ty = glib::Object::new(&[]).unwrap();
        view!($name = (_obj) {$($tt)+});
        view!($($ctt)*);
    };
    () => {};
    (@expand-build $obj:ident) => {};
    (@expand-build $obj:ident $member:ident : $wrap_tt:tt ($($inner_tt:tt)+), $($tt:tt)*) => {
        $obj.$member(
            $wrap_tt(view!(@expand-wrapped $($inner_tt)+))
        );
        view!(@expand-build $obj $($tt)*);
    };
    (@expand-wrapped $name:ident = $($ptt:tt)+) => {
        {
            view!($name = $($ptt)+);
            $name
        }
    };
    (@expand-wrapped $wrap_tt:tt ($name:ident = $($ptt:tt)+)) => {
        $wrap_tt({
            view!($name = $($ptt)+);
            $name
        })
    };
    (@expand-wrapped $wrap_tt:tt ($($inner_tt:tt)*)) => {
        $wrap_tt($($inner_tt)*)
    };
    (@expand-wrapped $($inner_tt:tt)*) => {
        $($inner_tt)*
    };
    (@expand-build $obj:ident $member:ident : $name:ident = $($itt:tt)+, $($tt:tt)*) => {
        view!($name = $($itt)+);
        $obj.$member($name);
        view!(@expand-build $obj $($tt)*);
    };
    (@expand-build $obj:ident $member:ident : $e:expr, $($tt:tt)*) => {
        $obj.$member($e);
        view!(@expand-build $obj $($tt)*);
    };
    (@expand-build $obj:ident $member:ident($($e:expr),+), $($tt:tt)*) => {
        $obj.$member($($e),+);
        view!(@expand-build $obj $($tt)*);
    };
    (@expand-build $obj:ident bind $prop:literal $from_ty:ident $source:literal, $($tt:tt)*) => {
        $from_ty.bind_property($source, &$obj, $prop).build();
    }
}
