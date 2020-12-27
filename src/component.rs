use futures::future::RemoteHandle;
use std::cell::Cell;
use std::thread_local;

thread_local! {
    static COMPONENT_ID_TRACKER: Cell<usize> = Cell::new(0);
}
pub fn new_component_id() -> usize {
    COMPONENT_ID_TRACKER.with(|static_id| {
        let id = static_id.get();
        static_id.set(id + 1);
        static_id.get()
    })
}

#[derive(Debug)]
pub struct Component<T: glib::IsA<gtk::Widget>, M> {
    id: usize,
    widget: T,
    chan: flume::Sender<M>,
    handle: RemoteHandle<()>,
}
impl<T: glib::IsA<gtk::Widget>, M> Component<T, M> {
    pub fn new(id: usize, widget: T, chan: flume::Sender<M>, handle: RemoteHandle<()>) -> Self {
        Component {
            id,
            widget,
            chan,
            handle,
        }
    }
    pub fn id(&self) -> usize {
        return self.id;
    }
    pub fn widget(&self) -> &T {
        return &self.widget;
    }
    pub fn chan(&self) -> flume::Sender<M> {
        return self.chan.clone();
    }
}
