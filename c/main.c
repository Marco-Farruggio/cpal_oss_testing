#include <sys/soundcard.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <errno.h>

int main(void) {
    int fd = open("/dev/dsp0", O_RDWR);
    if (fd < 0) { perror("open"); return 1; }

    struct oss_audioinfo ai;
    memset(&ai, 0, sizeof(ai));
    ai.dev = -1;

    if (ioctl(fd, SNDCTL_AUDIOINFO, &ai) < 0) {
        perror("SNDCTL_AUDIOINFO");
        return 2;
    }
    printf("name = %s\n", ai.name);
    return 0;
}