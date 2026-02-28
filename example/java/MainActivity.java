package com.example.zs;

import com.example.zs.R;

import android.app.Activity;
import android.os.Bundle;
import android.view.Surface;
import android.view.SurfaceHolder;
import android.view.SurfaceView;
import android.webkit.WebView;
import android.view.ViewGroup.LayoutParams;
import android.widget.LinearLayout;

import androidx.webkit.WebSettingsCompat;
import androidx.webkit.WebViewFeature;

public final class MainActivity extends Activity implements SurfaceHolder.Callback {
	private SurfaceView view;
	private WebView web;

	@Override
	protected void onCreate(Bundle b) {
		super.onCreate(b);

		LinearLayout root = new LinearLayout(this);
		root.setOrientation(LinearLayout.VERTICAL);

		view = new SurfaceView(this);
		view.getHolder().addCallback(this);
		root.addView(view, new LinearLayout.LayoutParams(LayoutParams.MATCH_PARENT, 0, 1f));

		web = new WebView(this);
		root.addView(web, new LinearLayout.LayoutParams(LayoutParams.MATCH_PARENT, 0, 1f));
		setContentView(root);

		webkitSmokeTest();
	}

	private void webkitSmokeTest() {
		if (WebViewFeature.isFeatureSupported(WebViewFeature.ALGORITHMIC_DARKENING)) {
			WebSettingsCompat.setAlgorithmicDarkeningAllowed(web.getSettings(), false);
		}

		web.getSettings().setJavaScriptEnabled(true);
		web.loadDataWithBaseURL(
			"https://example.test",
			"<html><body><h1>androidx.webkit smoke test</h1></body></html>",
			"text/html",
			"utf-8",
			null
		);
	}

	@Override
	public void surfaceCreated(SurfaceHolder holder) {
		Glue.surfaceCreated(holder.getSurface(), Math.max(1, view.getWidth()), Math.max(1, view.getHeight()));
	}

	@Override
	public void surfaceChanged(SurfaceHolder holder, int format, int width, int height) {
		Glue.surfaceChanged(holder.getSurface(), Math.max(1, width), Math.max(1, height));
	}

	@Override
	public void surfaceDestroyed(SurfaceHolder holder) {
		Glue.surfaceDestroyed(holder.getSurface());
	}

	@Override
	protected void onPause() {
		super.onPause();
		Glue.pause();
	}

	@Override
	protected void onResume() {
		super.onResume();
		Glue.resume();
	}
}

final class Glue {
	static { System.loadLibrary("main"); }

	static native void surfaceCreated(Surface surface, int width, int height);
	static native void surfaceChanged(Surface surface, int width, int height);
	static native void surfaceDestroyed(Surface surface);
	static native void pause();
	static native void resume();
}
