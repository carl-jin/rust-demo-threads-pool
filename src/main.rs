use std::{
    sync::{mpsc, Arc, Mutex},
    thread,
};
type Job = Box<dyn FnOnce() + Send + 'static>;
type thread_reciver = Arc<Mutex<mpsc::Receiver<Job>>>;

struct worker {
    id: usize,
    thread: thread::JoinHandle<()>,
}

impl worker {
    pub fn new(id: usize, reciver: thread_reciver) -> worker {
        let thread = thread::spawn(move || loop {
            let job_fn = { reciver.lock().unwrap().recv().unwrap() };
            println!("thread id: {} got a job", id);
            job_fn();
        });

        worker { id, thread }
    }
}

struct threads_pool {
    workers: Vec<worker>,
    sender: mpsc::Sender<Job>,
}

impl threads_pool {
    pub fn new(size: usize) -> threads_pool {
        let mut workers: Vec<worker> = Vec::with_capacity(size);

        let (sender, reciver) = mpsc::channel::<Job>();
        let reciver = Arc::new(Mutex::new(reciver));

        for i in 0..size {
            workers.push(worker::new(i, Arc::clone(&reciver)));
        }

        threads_pool { workers, sender }
    }

    pub fn execute<T>(&self, job: T)
    where
        T: FnOnce() + Send + 'static,
    {
        self.sender.send(Box::new(job)).unwrap();
    }
}

fn main() {
    let pool = threads_pool::new(4);

    for i in 0..10 {
        pool.execute(move || println!("{}", i))
    }

    for worker in pool.workers {
        worker.thread.join().unwrap();
    }
}
