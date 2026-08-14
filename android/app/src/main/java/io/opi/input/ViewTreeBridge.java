package io.opi.input;

import android.view.View;

import androidx.lifecycle.LifecycleOwner;
import androidx.lifecycle.ViewTreeLifecycleOwner;
import androidx.savedstate.SavedStateRegistryOwner;
import androidx.savedstate.ViewTreeSavedStateRegistryOwner;

/**
 * ViewTreeLifecycleOwner / ViewTreeSavedStateRegistryOwner 是 Kotlin object，
 * AGP 9 的 api-jar transform 剥掉 @Metadata 后 Kotlin 编译器无法解析（同 SavedStateRegistryFactory 情形），
 * Java 不受 metadata 限制，提供静态桥供 IME Service 的 ComposeView 挂接生命周期与状态注册。
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
