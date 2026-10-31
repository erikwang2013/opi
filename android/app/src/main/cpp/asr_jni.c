// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 薄 JNI 层：只做 Java 类型 <-> C ABI 的搬运，不碰识别逻辑。
// 链接 libopi_asr.so；sherpa/onnxruntime 由 APK 同目录的 .so 提供。

#include <jni.h>
#include <stdint.h>

extern void *opi_asr_create(const char *config_json);
extern void opi_asr_destroy(void *handle);
extern int32_t opi_asr_load_model(void *handle);
extern int32_t opi_asr_start(void *handle);
extern int32_t opi_asr_feed_pcm(void *handle, const uint8_t *data, int32_t len);
extern int32_t opi_asr_finish(void *handle);
extern int32_t opi_asr_cancel(void *handle);
extern int32_t opi_asr_poll_event(void *handle, char *buf, int32_t cap);
extern int32_t opi_asr_last_error(void *handle, char *buf, int32_t cap);
extern int32_t opi_asr_max_event_bytes(void);

static void *as_handle(jlong handle) {
    return (void *) (intptr_t) handle;
}

// 只在本层临时持有 Java 字节数组；返回时释放（poll/lastError 用 mode 0 回写）。
static jint with_buffer(JNIEnv *env, jbyteArray array, void *handle, int32_t (*call)(void *, char *, int32_t)) {
    if (array == NULL) {
        return -1;
    }
    jsize cap = (*env)->GetArrayLength(env, array);
    jbyte *buf = (*env)->GetByteArrayElements(env, array, NULL);
    if (buf == NULL) {
        return -1;
    }
    jint result = call(handle, (char *) buf, (int32_t) cap);
    (*env)->ReleaseByteArrayElements(env, array, buf, 0);
    return result;
}

JNIEXPORT jlong JNICALL
Java_xyz_erik_opi_asr_AsrNative_create(JNIEnv *env, jobject thiz, jstring config) {
    (void) thiz;
    if (config == NULL) {
        return 0;
    }
    const char *json = (*env)->GetStringUTFChars(env, config, NULL);
    if (json == NULL) {
        return 0;
    }
    void *handle = opi_asr_create(json);
    (*env)->ReleaseStringUTFChars(env, config, json);
    return (jlong) (intptr_t) handle;
}

JNIEXPORT void JNICALL
Java_xyz_erik_opi_asr_AsrNative_destroy(JNIEnv *env, jobject thiz, jlong handle) {
    (void) env;
    (void) thiz;
    opi_asr_destroy(as_handle(handle));
}

JNIEXPORT jint JNICALL
Java_xyz_erik_opi_asr_AsrNative_loadModel(JNIEnv *env, jobject thiz, jlong handle) {
    (void) env;
    (void) thiz;
    return opi_asr_load_model(as_handle(handle));
}

JNIEXPORT jint JNICALL
Java_xyz_erik_opi_asr_AsrNative_start(JNIEnv *env, jobject thiz, jlong handle) {
    (void) env;
    (void) thiz;
    return opi_asr_start(as_handle(handle));
}

JNIEXPORT jint JNICALL
Java_xyz_erik_opi_asr_AsrNative_finish(JNIEnv *env, jobject thiz, jlong handle) {
    (void) env;
    (void) thiz;
    return opi_asr_finish(as_handle(handle));
}

JNIEXPORT jint JNICALL
Java_xyz_erik_opi_asr_AsrNative_cancel(JNIEnv *env, jobject thiz, jlong handle) {
    (void) env;
    (void) thiz;
    return opi_asr_cancel(as_handle(handle));
}

JNIEXPORT jint JNICALL
Java_xyz_erik_opi_asr_AsrNative_feedPcm(JNIEnv *env, jobject thiz, jlong handle, jbyteArray data) {
    (void) thiz;
    if (data == NULL) {
        return -1;
    }
    jsize len = (*env)->GetArrayLength(env, data);
    jbyte *buf = (*env)->GetByteArrayElements(env, data, NULL);
    if (buf == NULL) {
        return -1;
    }
    jint result = opi_asr_feed_pcm(as_handle(handle), (const uint8_t *) buf, (int32_t) len);
    (*env)->ReleaseByteArrayElements(env, data, buf, JNI_ABORT);
    return result;
}

JNIEXPORT jint JNICALL
Java_xyz_erik_opi_asr_AsrNative_pollEvent(JNIEnv *env, jobject thiz, jlong handle, jbyteArray buf) {
    (void) thiz;
    return with_buffer(env, buf, as_handle(handle), opi_asr_poll_event);
}

JNIEXPORT jint JNICALL
Java_xyz_erik_opi_asr_AsrNative_lastError(JNIEnv *env, jobject thiz, jlong handle, jbyteArray buf) {
    (void) thiz;
    return with_buffer(env, buf, as_handle(handle), opi_asr_last_error);
}

JNIEXPORT jint JNICALL
Java_xyz_erik_opi_asr_AsrNative_maxEventBytes(JNIEnv *env, jobject thiz) {
    (void) env;
    (void) thiz;
    return opi_asr_max_event_bytes();
}
