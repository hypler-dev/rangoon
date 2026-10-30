/* Synthetic qualification probe. Never opens ordinary user data. */
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>

static int denied(void) { return errno == EPERM || errno == EACCES ? 42 : 2; }

int main(int argc, char **argv) {
    if (argc != 3) return 2;
    puts("rangoon.probe.v1");
    if (fflush(stdout) != 0) return 2;
    const char *kind = argv[1];
    if (strcmp(kind, "read") == 0 || strcmp(kind, "write") == 0) {
        int writing = strcmp(kind, "write") == 0;
        int fd = open(argv[2], writing ? O_WRONLY | O_CREAT | O_TRUNC : O_RDONLY, 0600);
        if (fd < 0) return denied();
        const char expected[] = "RANGOON-SYNTHETIC-CANARY\n";
        char data[sizeof(expected)] = {0};
        ssize_t count = writing ? write(fd, expected, sizeof(expected) - 1)
                                : read(fd, data, sizeof(data));
        int correct = count == (ssize_t)(sizeof(expected) - 1)
                      && (writing || memcmp(data, expected, sizeof(expected) - 1) == 0);
        if (close(fd) != 0 || !correct) return 2;
        return 0;
    }
    if (strcmp(kind, "tcp") == 0) {
        int fd = socket(AF_INET, SOCK_STREAM, 0);
        if (fd < 0) return denied();
        struct sockaddr_in address = {0};
        address.sin_family = AF_INET;
        address.sin_port = htons(9);
        address.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
        int result = connect(fd, (struct sockaddr *)&address, sizeof(address));
        int saved = errno;
        close(fd); errno = saved;
        return result < 0 ? denied() : 2;
    }
    if (strcmp(kind, "unix") == 0) {
        struct sockaddr_un address = {0};
        address.sun_family = AF_UNIX;
        if (strlen(argv[2]) >= sizeof(address.sun_path)) return 2;
        strcpy(address.sun_path, argv[2]);
        int fd = socket(AF_UNIX, SOCK_STREAM, 0);
        if (fd < 0) return denied();
        int result = connect(fd, (struct sockaddr *)&address, sizeof(address));
        int saved = errno;
        close(fd); errno = saved;
        return result < 0 ? denied() : 2;
    }
    if (strcmp(kind, "exec") == 0) {
        execl("/usr/bin/true", "true", (char *)NULL);
        return denied();
    }
    return 2;
}
