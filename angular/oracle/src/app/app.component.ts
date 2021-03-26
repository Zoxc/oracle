import { Component } from '@angular/core';
import { ActivatedRoute, Router } from '@angular/router';

@Component({
  selector: 'app-root',
  templateUrl: './app.component.html',
  styleUrls: ['./app.component.scss']
})
export class AppComponent {
  url = null;
  isCollapsed = false;

  constructor(private route: ActivatedRoute,
    private router: Router) {
  }

  ngOnInit() {
    console.log(this.router.url);
    this.router.url;
    this.route.url.subscribe(url => {
      console.log(url);
      this.url = url;
    });
  }
}
