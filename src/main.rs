use std::{
    sync::{mpsc, Arc, Condvar, Mutex},
    thread, time,
};

type Job = Box<dyn FnOnce() + Send + 'static>;

struct ShareData {
    reciver: Mutex<mpsc::Receiver<Job>>,
    queue_count: Mutex<usize>,
    running_count: Mutex<usize>,
    threads_limits: Mutex<usize>,
    cond_lock: Mutex<()>,
    cond: Condvar,
}

impl ShareData {
    pub fn has_tasks(&self) -> bool {
        *self.queue_count.lock().unwrap() > 0 || *self.running_count.lock().unwrap() > 0
    }

    pub fn job_done(&self) {
        if !self.has_tasks() {
            self.cond_lock.lock().unwrap();
            self.cond.notify_all()
        }
    }
}

struct Worker {
    id: usize,
    thread: thread::JoinHandle<()>,
}

impl Worker {
    pub fn new(id: usize, share_data: Arc<ShareData>) -> Worker {
        let thread = thread::spawn(move || loop {
            {
                if *share_data.running_count.lock().unwrap()
                    >= *share_data.threads_limits.lock().unwrap()
                {
                    break;
                }
            }
            let job_fn = { share_data.reciver.lock().unwrap().recv().unwrap() };
            {
                *share_data.queue_count.lock().unwrap() -= 1;
            }
            {
                *share_data.running_count.lock().unwrap() += 1;
            }
            print!("线程 {id} 开始执行任务……\n");
            job_fn();
            {
                *share_data.running_count.lock().unwrap() -= 1;
            }
            share_data.job_done();
        });

        Worker { id, thread }
    }
}

struct ThreadsPool {
    sender: mpsc::Sender<Job>,
    workers: Vec<Worker>,
    data: Arc<ShareData>,
}

impl ThreadsPool {
    pub fn new(size: usize) -> ThreadsPool {
        //  创建 channel
        let (sender, reciver) = mpsc::channel::<Job>();
        let mut workers: Vec<Worker> = Vec::with_capacity(size);

        let share_data = Arc::new(ShareData {
            reciver: Mutex::new(reciver),
            queue_count: Mutex::new(0),
            running_count: Mutex::new(0),
            threads_limits: Mutex::new(size),
            cond_lock: Mutex::new(()),
            cond: Condvar::new(),
        });

        for i in 0..size {
            workers.push(Worker::new(i, Arc::clone(&share_data)));
        }

        ThreadsPool {
            sender,
            workers,
            data: share_data,
        }
    }

    pub fn execute<T>(&self, job: T)
    where
        T: FnOnce() + Send + 'static,
    {
        {
            *self.data.queue_count.lock().unwrap() += 1;
        }
        self.sender.send(Box::new(job)).unwrap()
    }

    pub fn join(&self) {
        while self.data.has_tasks() {
            let cond_lock = self.data.cond_lock.lock().unwrap();
            self.data.cond.wait(cond_lock).unwrap();
        }
    }

    pub fn set_threads_num(&mut self, size: usize) {
        let prev_limit = *self.data.threads_limits.lock().unwrap();

        if let Some(n) = size.checked_sub(prev_limit) {
            for i in 0..n {
                self.workers
                    .push(Worker::new(i + &prev_limit, Arc::clone(&self.data)))
            }
        }

        *self.data.threads_limits.lock().unwrap() = size;
    }
}

fn main() {
    let mut pool = ThreadsPool::new(4);

    for i in 0..40 {
        pool.execute(move || {
            thread::sleep(time::Duration::from_secs(1));
            println!("{i}")
        })
    }

    thread::sleep(time::Duration::from_secs(5));
    pool.set_threads_num(10);

    pool.join();
}
