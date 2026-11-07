#ifndef DEMO_290_CALLBACK_REGISTRY_H
#define DEMO_290_CALLBACK_REGISTRY_H

typedef int (*Demo290Callback)(int);
int host_register_callback(Demo290Callback cb);

#endif
