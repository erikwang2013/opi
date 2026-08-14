package io.opi.input.ime

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier

/** IME 面板根（骨架）：A3/A4 填键盘/候选栏/引擎路由。 */
@Composable
fun ImeScreen(state: ImeState) {
    Column(
        modifier = Modifier.fillMaxSize(),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Text("OPI")
    }
}
