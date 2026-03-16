package com.omnipresent

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import com.omnipresent.ui.AppNavigation
import com.omnipresent.ui.theme.OmnipresentTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            OmnipresentTheme {
                AppNavigation()
            }
        }
    }
}
