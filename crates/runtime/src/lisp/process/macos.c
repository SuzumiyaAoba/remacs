/* Native macOS process metadata. Keep the exported layout in sync with macos.rs. */
#include <sys/types.h>
#include <sys/param.h>
#include <sys/proc.h>
#include <sys/sysctl.h>
#include <sys/user.h>
#include <sys/stat.h>
#include <libproc.h>
#include <mach/mach_time.h>
#include <pwd.h>
#include <grp.h>
#include <stdint.h>
#include <string.h>
#include <stdlib.h>

struct remacs_process_info {
    uint64_t flags, user_ns, system_ns, pageins, virtual_size, resident_size;
    int64_t start_sec, start_usec;
    uint32_t euid, egid;
    int32_t ppid, pgrp, tpgid, nice, thcount;
    char state[8], comm[4096], user[256], group[256], tty[1024];
};

enum { BASE = 1, USER = 2, GROUP = 4, USAGE = 8, TASK = 16, ARGS = 32, TTY = 64 };

int remacs_macos_process_attributes(int pid, struct remacs_process_info *out,
                                   size_t out_size, char *args, size_t args_size) {
    if (out_size != sizeof *out) return 0;
    memset(out, 0, sizeof *out);
    int mib[] = { CTL_KERN, KERN_PROC, KERN_PROC_PID, pid };
    struct kinfo_proc proc;
    size_t size = sizeof proc;
    if (sysctl(mib, 4, &proc, &size, NULL, 0) != 0 || size != sizeof proc) return 0;
    out->flags = BASE;
    out->euid = proc.kp_eproc.e_ucred.cr_uid;
    out->egid = proc.kp_eproc.e_pcred.p_svgid;
    out->ppid = proc.kp_eproc.e_ppid;
    out->pgrp = proc.kp_eproc.e_pgid;
    out->tpgid = proc.kp_eproc.e_tpgid;
    out->nice = proc.kp_proc.p_nice;
    out->start_sec = proc.kp_proc.p_starttime.tv_sec;
    out->start_usec = proc.kp_proc.p_starttime.tv_usec;
    switch (proc.kp_proc.p_stat) {
        case SRUN: out->state[0] = 'R'; break;
        case SSLEEP: out->state[0] = 'S'; break;
        case SZOMB: out->state[0] = 'Z'; break;
        case SSTOP: out->state[0] = 'T'; break;
        case SIDL: out->state[0] = 'I'; break;
    }
    char path[PROC_PIDPATHINFO_MAXSIZE];
    if (proc_pidpath(pid, path, sizeof path) > 0) {
        char *base = strrchr(path, '/');
        strlcpy(out->comm, base ? base + 1 : path, sizeof out->comm);
    } else strlcpy(out->comm, proc.kp_proc.p_comm, sizeof out->comm);

    char scratch[16384];
    struct passwd pw, *pw_result;
    if (getpwuid_r(out->euid, &pw, scratch, sizeof scratch, &pw_result) == 0 && pw_result) {
        strlcpy(out->user, pw_result->pw_name, sizeof out->user);
        out->flags |= USER;
    }
    struct group gr, *gr_result;
    if (getgrgid_r(out->egid, &gr, scratch, sizeof scratch, &gr_result) == 0 && gr_result) {
        strlcpy(out->group, gr_result->gr_name, sizeof out->group);
        out->flags |= GROUP;
    }
    if (proc.kp_eproc.e_tdev != NODEV &&
        devname_r(proc.kp_eproc.e_tdev, S_IFCHR, out->tty, sizeof out->tty) != NULL)
        out->flags |= TTY;

    struct rusage_info_v0 usage;
    if (proc_pid_rusage(pid, RUSAGE_INFO_V0, (rusage_info_t *)&usage) == 0) {
        mach_timebase_info_data_t scale;
        if (mach_timebase_info(&scale) == KERN_SUCCESS && scale.denom != 0) {
            out->user_ns = (__uint128_t)usage.ri_user_time * scale.numer / scale.denom;
            out->system_ns = (__uint128_t)usage.ri_system_time * scale.numer / scale.denom;
            out->pageins = usage.ri_pageins;
            out->flags |= USAGE;
        }
    }
    struct proc_taskinfo task;
    if (proc_pidinfo(pid, PROC_PIDTASKINFO, 0, &task, sizeof task) == sizeof task) {
        out->virtual_size = task.pti_virtual_size;
        out->resident_size = task.pti_resident_size;
        out->thcount = task.pti_threadnum;
        out->flags |= TASK;
    }
    if (args_size > sizeof(int)) {
        mib[1] = KERN_PROCARGS2;
        mib[2] = pid;
        size = args_size;
        if (sysctl(mib, 3, args, &size, NULL, 0) == 0 && size >= sizeof(int)) {
            int argc;
            memcpy(&argc, args, sizeof argc);
            char *end = args + size;
            char *start = memchr(args + sizeof(int), 0, size - sizeof(int));
            if (start) {
                while (start < end && !*start) start++;
                char *cursor = start;
                int count = 0;
                while (count < argc && cursor < end) {
                    char *nul = memchr(cursor, 0, end - cursor);
                    if (!nul) break;
                    cursor = nul + 1;
                    count++;
                }
                if (argc >= 0 && count == argc && cursor > start) {
                    size_t len = cursor - start;
                    for (size_t j = 0; j + 1 < len; j++)
                        if (!start[j]) start[j] = ' ';
                    memmove(args, start, len);
                    out->flags |= ARGS;
                }
            }
        }
    }
    return 1;
}

/* Return the required PID capacity when pids is NULL, or fill GNU's list order. */
size_t remacs_macos_system_processes(int32_t *pids, size_t capacity) {
    int mib[] = { CTL_KERN, KERN_PROC, KERN_PROC_ALL };
    size_t size = 0;
    if (sysctl(mib, 3, NULL, &size, NULL, 0) != 0 || size == 0) return 0;
    size_t count = size / sizeof(struct kinfo_proc);
    if (!pids) return count;
    if (count > capacity) return 0;
    struct kinfo_proc *processes = malloc(size);
    if (!processes) return 0;
    if (sysctl(mib, 3, processes, &size, NULL, 0) != 0) {
        free(processes);
        return 0;
    }
    count = size / sizeof *processes;
    if (count > capacity) {
        free(processes);
        return 0;
    }
    for (size_t i = 0; i < count; i++)
        pids[count - i - 1] = processes[i].kp_proc.p_pid;
    free(processes);
    return count;
}
