package io.opi.input;

import androidx.savedstate.SavedStateRegistry;

import java.lang.reflect.Field;

/**
 * savedstate 1.2.1 的 SavedStateRegistry() 构造为 Kotlin internal（Kotlin 侧不可调），
 * performRestore 在 AGP 9 transform 的 api jar 中被 mangle（performRestore$savedstate_release），
 * 源码无法调用；而 Compose 的 AndroidComposeView 要求 ViewTree 上有已 restore 的
 * SavedStateRegistryOwner（onAttachedToWindow 检查 + consumeRestoredStateForKey），
 * 否则直接抛 IllegalStateException。Java 不受 internal 限制可构造实例，isRestored 字段
 * 无 mangling，反射置位跳过 performRestore（IME 无持久化状态，等效空 restore）。
 */
public final class SavedStateRegistryFactory {
    private SavedStateRegistryFactory() {
    }

    public static SavedStateRegistry createRestored() {
        SavedStateRegistry registry = new SavedStateRegistry();
        try {
            Field restored = SavedStateRegistry.class.getDeclaredField("isRestored");
            restored.setAccessible(true);
            restored.setBoolean(registry, true);
        } catch (Exception e) {
            throw new RuntimeException("savedstate isRestored 反射失败", e);
        }
        return registry;
    }
}
