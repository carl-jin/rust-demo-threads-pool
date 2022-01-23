use core::num;
use std::{
    os::unix::raw::pthread_t,
    sync::{mpsc, Arc, Condvar, Mutex},
    thread, time,
};
type Job = Box<dyn FnOnce() + Send + 'static>;
type thread_reciver = Arc<Mutex<mpsc::Receiver<Job>>>;

struct worker {
    id: usize,
    thread: thread::JoinHandle<()>,
}

struct sharedData {
    reciver: Mutex<mpsc::Receiver<Job>>,
    queue_cnt: Mutex<u32>,
    active_cnt: Mutex<u32>,
    max_threads_cnt: Mutex<usize>,
    cond_guard: Mutex<bool>,
    cond: Condvar,
}

impl worker {
    pub fn new(id: usize, share_data: Arc<sharedData>) -> worker {
        let thread = thread::spawn(move || loop {
            {
                if usize::try_from(*share_data.active_cnt.lock().unwrap()).unwrap()
                    >= *share_data.max_threads_cnt.lock().unwrap()
                {
                    break;
                }
            }

            {
                if !share_data.has_tasks() {
                    break;
                }
            }

            let job_fn = {
                share_data
                    .reciver
                    .lock()
                    .expect("获取锁错误")
                    .recv()
                    .expect("获取消息错误")
            };
            {
                *share_data.queue_cnt.lock().unwrap() -= 1;
            }
            {
                *share_data.active_cnt.lock().unwrap() += 1;
            }
            println!("thread id: {} got a job", id);
            job_fn();
            {
                *share_data.active_cnt.lock().unwrap() -= 1;
            }

            share_data.notifiy_if_no_tasks();
        });

        worker { id, thread }
    }
}

impl sharedData {
    //  判断当前是否还有任务
    pub fn has_tasks(&self) -> bool {
        let queue_cnt = *self.queue_cnt.lock().unwrap();
        let active_cnt = *self.active_cnt.lock().unwrap();

        queue_cnt > 0 || active_cnt > 0
    }

    pub fn notifiy_if_no_tasks(&self) {
        if !self.has_tasks() {
            *self.cond_guard.lock().expect("can't lock cond_guard");
            self.cond.notify_all();
        }
    }
}

struct threads_pool {
    sender: mpsc::Sender<Job>,
    data: Arc<sharedData>,
    workers: Vec<worker>,
}

impl threads_pool {
    pub fn new(size: usize) -> threads_pool {
        let mut workers: Vec<worker> = Vec::with_capacity(size);
        let (sender, reciver) = mpsc::channel::<Job>();
        let reciver = Mutex::new(reciver);
        let share_data = Arc::new(sharedData {
            reciver,
            queue_cnt: Mutex::new(0),
            active_cnt: Mutex::new(0),
            max_threads_cnt: Mutex::new(size),
            cond_guard: Mutex::new(false),
            cond: Condvar::new(),
        });

        for i in 0..size {
            workers.push(worker::new(i, Arc::clone(&share_data)));
        }

        threads_pool {
            data: share_data,
            sender,
            workers,
        }
    }

    pub fn execute<T>(&self, job: T)
    where
        T: FnOnce() + Send + 'static,
    {
        {
            *self.data.queue_cnt.lock().unwrap() += 1;
        }
        self.sender.send(Box::new(job)).unwrap();
    }

    pub fn join(&self) {
        let mut guard = self.data.cond_guard.lock().expect("can't lock cond_guard");
        while self.data.has_tasks() {
            guard = self.data.cond.wait(guard).unwrap();
            println!("执行完毕");
        }
    }

    pub fn set_threads_num(&mut self, size: usize) {
        let mut prev_cnt = size;
        {
            let mut p_thread_cnt = self.data.max_threads_cnt.lock().unwrap();
            prev_cnt = *p_thread_cnt;
            *p_thread_cnt = size;
        }

        if let Some(n) = size.checked_sub(prev_cnt) {
            for _ in 0..n {
                self.workers
                    .push(worker::new(n + prev_cnt, Arc::clone(&self.data)));
            }
        }
    }
}

struct NumberTest {
    count: Mutex<u32>,
}

fn main() {
    let mut pool = threads_pool::new(4);

    for i in 0..10 {
        pool.execute(move || {
            thread::sleep(time::Duration::from_secs(1));
            println!("{i}")
        })
    }

    thread::sleep(time::Duration::from_secs(1));
    pool.set_threads_num(2);

    pool.join();
}
