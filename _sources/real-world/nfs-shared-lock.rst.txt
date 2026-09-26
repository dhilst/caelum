NFS Shared Lock
===============

This example comes from a real data pipeline. It shows how to use Caelum to
convince yourself that a small, coordination-free locking protocol can't
corrupt data and can't get stuck.

The problem
-----------

A large pipeline runs its jobs on several machines. The jobs produce huge
amounts of data, and copying that data between machines is too expensive.
The machine that produces a dataset is not the one that validates it, and
validation may write new files next to the data it checks.

The solution is a **shared scratch space**: every machine mounts the same NFS
export, and jobs read and write there directly. To keep things simple, jobs
**don't talk to each other** and there is no central coordinator. Instead:

- A job **locks a directory** on NFS before writing to it. Anyone may read, but
  writing happens only while holding the lock. This is a convention the
  pipeline code enforces and checks.
- The lock is a file in the directory recording **who** holds it: the
  hostname and the process identity (pid, process start time and boot id).

Two things must never happen:

- **Data corruption.** Two runs must never write the same directory at once.
  If a bug makes a job try, it should **fail loudly** (wait or time out)
  rather than silently overwrite someone else's data. This is the main safety
  property.
- **Deadlock after a crash.** If a job crashes while holding the lock, the
  lock file stays behind. The next run of that job must still be able to
  proceed, or the pipeline is stuck forever.

The key fact that makes recovery safe without any coordination: **a given job
always runs on the same host.** A host can tell for certain whether one of its
own processes is dead, by checking that the recorded pid no longer exists, or
that it now belongs to a different process (different start time or boot id).
So when the next run finds a lock left by a dead process *on its own host*, it
can take the lock over. A process on *another* host can't make that check: to
it, a crashed holder and a slow holder look the same, so it must never take
the lock over. It can only wait.

.. note::

   Technical details: In **re-exported** NFS mount, the kernel refuses file
   locks, ``flock``/``fcntl`` fail with ``EOPNOTSUPP``, so the locking has to
   be implemented from operations that NFSv4 makes atomic on the server:
   ``mkdir`` and ``rename``.

The protocol
------------

``acquire`` (repeat until it succeeds, or give up after a timeout):

1. If there is no lock file, atomically create it with your
   ``(host, pid)`` record. If the create fails, someone else won the race.
2. If there is a lock file whose host is **your** host and whose process is
   dead, atomically replace it with your record. That's the crash recovery.
3. Otherwise someone is working. Wait.

``release``: delete the lock file.

The model
---------

The spec below models the lock of **one** directory: every process in it is a
run of the same job that wants that directory. Locks on different directories
don't interact, so one is enough.

- **Hosts and processes.** Two hosts, ``local`` and ``remote``, and three
  process slots. ``local`` has two slots, so two runs on the same host can
  race to recover a crashed lock; ``remote`` has one slot and plays the
  "other host". Where each process runs is a static fact, ``host[p]``, fixed
  in its own ``init`` block and kept ``unchanged`` by every transition.
- **Process state.** Each slot is ``free`` (not running, finished, or
  crashed), ``waiting`` (inside ``acquire``) or ``holding`` (owns the lock and
  may write).
- **The lock file.** ``rec`` is ``none`` (no lock file), ``live`` (held by a
  running process) or ``stale`` (held by a process that crashed). ``owner`` is
  the host recorded in the file. The difference between ``live`` and
  ``stale`` is only observable from the owner's host.
- **Transitions.** ``spawn`` starts a run, ``acquire`` takes a free lock or
  recovers a stale one on its own host, ``release`` finishes normally, and
  ``crash`` dies while holding the lock, leaving a stale record.

**Assumptions** are what the protocol relies on from the environment:

- **A0 Same-host liveness check (safety).** Only the recorded host can decide
  that the holder is dead, so only it may recover. This is the guard
  ``owner = host[p]`` in ``acquire``.
- **A1 Atomic create/replace.** Checking the lock file and writing it happen
  as one step on NFS (e.g. exclusive create, or ``link()`` + link count
  check).
- **A2 Processes finish.** A holder eventually releases the lock or crashes.
- **A3 Processes eventually succeed.** A waiting run eventually gets the lock.
  Timeouts are left out: a run that gives up never touched the lock file, so
  it can't affect safety.
- **A4 A crashed host comes back.** It keeps starting new runs, so someone is
  there to recover its stale lock.

A2 and A3 are the ``fairness`` block. A4 needs no declaration: the model has
no idle step, so when everyone is waiting on a stale lock, starting a new run
on the crash host is the only possible move.

Click **Check** to verify it in your browser:

.. code-block:: lum

   module examples.scratch_lock

   // Distributed locking over a shared scratch space.
   //
   // Concrete case: several hosts mount the same NFS export. Each work w has a
   // lock file in the mount holding (host, pid). This model is the lock of ONE
   // work w; every process is an instance of w (works don't interact). If q
   // starts while p is running, q must wait.
   //
   // The lock file is the only shared state (no host-to-host messages). Any host
   // can take a free lock. If the holder crashes its record stays behind, stale.
   //
   // Structure:
   //   H = {local, remote},  P = {0, 1, 2},  host[p] = the host p runs at
   //   local runs pids 0, 1 (same-host recovery race); remote runs pid 2
   //
   // Lock record:  rec ∈ {none, live, stale},  owner = host field of the record.
   // Pids are unique (pid + start time), so only a crash makes a record stale.
   //
   // Assumptions:
   //   A0 same-host liveness (safety): only the holder's host can see that the
   //      pid is dead (kill(pid, 0)); another host can't tell "crashed" from
   //      "still working". So only the crash host may recover a stale lock —
   //      encoded in the guard of `acquire`.
   //   A1 create and recover are atomic (CAS) on the scratch space
   //   A2 processes finish: a holder eventually releases or crashes
   //   A3 processes eventually succeed: a waiter eventually acquires
   //      (a waiter that gives up leaves no trace in the lock, so safety is
   //       unaffected by dropping timeouts)
   //   A4 hosts keep starting processes (a crashed host comes back) — implicit:
   //      there is no idle step, so when everyone waits on a stale lock the
   //      crash host's spawn is the only move

   type Host = enum { local, remote }
   type Proc = 0..2

   type PState = enum { free, waiting, holding }  // free = slot empty (never started / done / crashed)
   type Record = enum { none, live, stale }

   let st[p ∈ Proc] ∈ PState
   let rec   ∈ Record
   let owner ∈ Host
   let host[p ∈ Proc] ∈ Host   // static fact, fixed in init; every transition keeps it unchanged

   // Static facts: where each process runs (never changes).
   init {
     host[0] = local ∧
     host[1] = local ∧
     host[2] = remote
   }

   // Initial state: nobody running, no lock.
   init {
     (∀ p ∈ Proc: st[p] = free) ∧ rec = none ∧ owner = local
   }

   // A new process starts and calls acquire.
   transition spawn(p ∈ Proc) {
     st[p] = free ∧
     st[p]' = waiting ∧
     unchanged(st except p, rec, owner, host)
   }

   // acquire → ok (atomic CAS, A1): take a free lock (any host), or recover a
   // stale one — only on the owner's host (A0).
   transition acquire(p ∈ Proc) {
     st[p] = waiting ∧
     (rec = none ∨ (rec = stale ∧ owner = host[p])) ∧
     st[p]' = holding ∧
     rec' = live ∧
     owner' = host[p] ∧
     unchanged(st except p, host)
   }

   // The holder finishes and releases the lock.
   transition release(p ∈ Proc) {
     st[p] = holding ∧
     st[p]' = free ∧
     rec' = none ∧
     unchanged(st except p, owner, host)
   }

   // The holder crashes: its record is left behind, stale.
   transition crash(p ∈ Proc) {
     st[p] = holding ∧
     st[p]' = free ∧
     rec' = stale ∧
     unchanged(st except p, owner, host)
   }

   fairness {
     weak   release           // A2 (a crash also ends the holder)
     strong acquire           // A3
   }

   // ── Safety: no data races ───────────────────────────────────────────────────

   // S1. At most one process holds the lock.
   property mutual_exclusion {
     □ (∀ p ∈ Proc: ∀ q ∈ Proc: (p ≠ q ∧ st[p] = holding) → st[q] ≠ holding)
   }

   // S2. If q starts while p holds the lock, q waits until p is gone.
   property second_instance_waits {
     □ (∀ p ∈ Proc: ∀ q ∈ Proc:
          (p ≠ q ∧ st[p] = holding ∧ st[q] = waiting) →
            ◯ (st[q] = waiting ∨ st[p] = free))
   }

   // S3. The lock file names the holder's host.
   property record_matches_holder {
     □ (∀ p ∈ Proc: st[p] = holding → (rec = live ∧ owner = host[p]))
   }

   // ── Liveness: crash is recoverable ──────────────────────────────────────────

   // L1. A crash while holding the lock doesn't deadlock: the crash host
   // eventually takes the lock back.
   property crash_recoverable {
     □ (∀ h ∈ Host: (rec = stale ∧ owner = h) → ◇ (rec = live ∧ owner = h))
   }

   // ── Liveness: work progresses ───────────────────────────────────────────────

   // L2. Every waiting process eventually gets the lock.
   property work_progresses {
     □ (∀ p ∈ Proc: st[p] = waiting → ◇ (st[p] = holding))
   }

**Properties:**

- ``mutual_exclusion`` (safety, *no data corruption*): at most one process
  holds the lock, across all hosts.
- ``second_instance_waits`` (safety): if a run starts while another one holds
  the lock, it waits until the holder has released it or crashed. It never
  pushes in.
- ``record_matches_holder`` (safety): the lock file always names the holder's
  host, so the liveness check in A0 is asked of the right host.
- ``crash_recoverable`` (liveness, *no deadlock*): a lock left behind by a
  crash is eventually taken back by a process on the crash host.
- ``work_progresses`` (liveness): every waiting run eventually gets the lock.

**What the model doesn't cover.** If the crash host never comes back (A4 is
false), its stale lock stays forever. No protocol without communication can
fix this, because another host can't tell a dead host from a slow one. That
case needs an operator (me) to remove the lock by hand, since a host failure
shows up in the pipeline, nothing fails silently. The model also assumes that
jobs honour the write-only-under-lock convention; that part is enforced in the
client code; enforced by testing, not by the lock library.

See ``examples/scratch_lock/scratch_lock.lum`` in the repository for the spec file.
