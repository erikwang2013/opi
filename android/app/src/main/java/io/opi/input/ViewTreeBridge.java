package io.opi.input;

import android.view.View;

import androidx.lifecycle.LifecycleOwner;
import androidx.lifecycle.ViewTreeLifecycleOwner;
import androidx.savedstate.SavedStateRegistryOwner;
import androidx.savedstate.ViewTreeSavedStateRegistryOwner;

/**
 * ViewTreeLifecycleOwner / ViewTreeSavedStateRegistryOwner 是 Kotlin object，
 * AGP 9 的 api-jar transform 剥掉 @Metadata 后 Kotlin 编译器无法解析，
 * Java 不受 metadata 限制，提供静态桥供 IME Service 挂接生命周期与状态注册。
 * SavedStateRegistryOwner 必须挂：Compose 1.7+ 的 AndroidComposeView 要求
 * propagateViewTreeSavedStateRegistryOwner，否则 onAttachedToWindow 直接抛异常；
 * registry 需为已 restore 状态（见 SavedStateRegistryFactory），否则 consume 即崩。
 */
public final class ViewTreeBridge {
    private ViewTreeBridge() {
    }

    public static void setLifecycleOwner(View view, LifecycleOwner owner) {
        ViewTreeLifecycleOwner.set(view, owner);
    }

    public static void setSavedStateRegistryOwner(View view, SavedStateRegistryOwner owner) {
        ViewTreeSavedStateRegistryOwner.set(view, owner);
    }
}
