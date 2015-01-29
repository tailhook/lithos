#include <sys/prctl.h>
#include <sys/mount.h>
#include <fcntl.h>
#include <alloca.h>
#include <unistd.h>
#include <signal.h>
#include <sched.h>
#include <unistd.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <math.h>
#include <grp.h>

// Glibc has a function, but doesn't declare any header for it
int pivot_root(const char *new_root, const char *put_old);

typedef struct {
    int namespaces;
    int pipe_reader;
    int user_id;
    int group_id;
    int restore_sigmask;
    const char *logprefix;
    const char *fs_root;
    const char *tmp_old_root;
    const char *old_root_relative;
    const char *exec_path;
    char ** const exec_args;
    char ** const exec_environ;
    const char *workdir;
    const char *output;
} CCommand;

typedef struct {
    int signo;
    pid_t pid;
    int status;
} CSignalInfo;

static void _run_container(CCommand *cmd) {
    prctl(PR_SET_PDEATHSIG, SIGKILL, 0, 0, 0);

    //  Wait for user namespace to be set up
    int rc;
    char val[1];
    do {
        rc = read(cmd->pipe_reader, val, 1);
    } while(rc < 0 && (errno == EINTR || errno == EAGAIN));
    if(rc < 0) {
        fprintf(stderr, "%s Error reading from parent's pipe: %m\n",
            cmd->logprefix);
        abort();
    }
    close(cmd->pipe_reader);

    if(cmd->fs_root) {
        if(setuid(0)) {
            fprintf(stderr, "%s Can't become root, to apply chroot: %m\n",
                cmd->logprefix);
            abort();
        }
        if(chdir(cmd->fs_root)) {
            fprintf(stderr, "%s Error changing workdir to the root %s: %m\n",
                cmd->logprefix, cmd->fs_root);
            abort();
        }
        if(cmd->tmp_old_root) {
            if(pivot_root(cmd->fs_root, cmd->tmp_old_root)) {
                fprintf(stderr, "%s Error changing root %s(%s): %m\n",
                    cmd->logprefix, cmd->fs_root, cmd->tmp_old_root);
                abort();
            }
            if(mount("none", cmd->old_root_relative, NULL,
                MS_REC|MS_PRIVATE, NULL))
            {
                fprintf(stderr, "%s Can't make mountpoint private: %m\n",
                    cmd->logprefix);
                abort();
            }
            if(umount2(cmd->old_root_relative, MNT_DETACH)) {
                fprintf(stderr, "%s Can't unmount old root: %m\n",
                    cmd->logprefix);
                abort();
            }
        } else {
            if(chroot(cmd->fs_root)) {
                fprintf(stderr, "%s Error changing root %s: %m\n",
                    cmd->logprefix, cmd->fs_root);
                abort();
            }
        }
    }
    if(chdir(cmd->workdir)) {
        fprintf(stderr, "%s Error changing workdir %s: %m\n",
            cmd->logprefix, cmd->workdir);
        abort();
    }
    if(setgid(cmd->group_id)) {
        fprintf(stderr, "%s Error setting group id %d: %m\n",
            cmd->logprefix, cmd->group_id);
        abort();
    }
    // Shouldn't we set zero supplemental groups
    if(setgroups(1, &cmd->group_id)) {
        fprintf(stderr, "%s Error setting groups: %m\n",
            cmd->logprefix);
        abort();
    }
    if(setuid(cmd->user_id)) {
        fprintf(stderr, "%s Error setting userid %d: %m\n",
            cmd->logprefix, cmd->user_id);
        abort();
    }
    if(cmd->output) {
        int fd = open(cmd->output, O_CREAT|O_WRONLY|O_APPEND, 0666);
        if(fd < 0) {
            fprintf(stderr, "%s Can't open file %s: %m\n",
                cmd->logprefix, cmd->output);
            abort();
        }
        if(fd != 1 && dup2(fd, 1) != 1 ||
           fd != 2 && dup2(fd, 2) != 2) {
            fprintf(stderr, "%s Can't duplicate fd for stdio: %m\n",
                cmd->logprefix);
            abort();
        }
        if(fd != 1 && fd != 2) {
            close(fd);
        }
    }
    if(cmd->restore_sigmask) {
        sigset_t mask;
        sigfillset(&mask);
        sigprocmask(SIG_UNBLOCK, &mask, NULL);
    }
    (void)execve(cmd->exec_path, cmd->exec_args, cmd->exec_environ);
    _exit(127);
}

pid_t execute_command(CCommand *cmd) {
    size_t stack_size = sysconf(_SC_PAGESIZE);
    void *stack = alloca(stack_size);

    return clone((int (*)(void*))_run_container,
        stack + stack_size,
        cmd->namespaces|SIGCHLD,
        cmd);
}

void block_all_signals() {
    sigset_t mask;
    sigfillset(&mask);
    sigprocmask(SIG_BLOCK, &mask, NULL);
}

int wait_any_signal(CSignalInfo *sig, double timeo) {
    struct timespec ts = {
        .tv_sec = (long)timeo,
        .tv_nsec = (int)ceil((timeo - floor(timeo))*1000000000),
    };
    sigset_t mask;
    sigfillset(&mask);
    while(1) {
        siginfo_t native_info;
        int rc;
        if(timeo >= 0) {
            rc = sigtimedwait(&mask, &native_info, &ts);
        } else {
            rc = sigwaitinfo(&mask, &native_info);
        }
        if(rc < 0){
            if(errno == EINTR) {
                return 1;
            } else if(errno == EAGAIN) {
                return 1;
            } else {
                fprintf(stderr, "Wrong error code for sigwaitinfo: %m\n");
                abort();
            }
        }
        sig->signo = native_info.si_signo;
        sig->pid = native_info.si_pid;
        sig->status = native_info.si_code == CLD_EXITED
            ? native_info.si_status
            : 128 + native_info.si_status;  // Wrapped signal
        return 0;
    }
}

