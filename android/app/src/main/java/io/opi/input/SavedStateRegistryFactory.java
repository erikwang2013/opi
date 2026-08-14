package io.opi.input;

import androidx.savedstate.SavedStateRegistry;

/**
 * savedstate 1.2.1 的 SavedStateRegistry() 构造为 Kotlin internal（Kotlin 侧不可调），
 * Java 不受 internal 限制，提供最小实例供 IME Service 的 SavedStateRegistryOwner 使用。
 */
public final class SavedStateRegistryFactory {
    private SavedStateRegistryFactory() {
    }

    public static SavedStateRegistry create() {
        return new SavedStateRegistry();
    }
}
