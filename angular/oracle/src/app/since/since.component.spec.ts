import { ComponentFixture, TestBed } from '@angular/core/testing';

import { SinceComponent } from './since.component';

describe('SinceComponent', () => {
  let component: SinceComponent;
  let fixture: ComponentFixture<SinceComponent>;

  beforeEach(async () => {
    await TestBed.configureTestingModule({
      declarations: [ SinceComponent ]
    })
    .compileComponents();
  });

  beforeEach(() => {
    fixture = TestBed.createComponent(SinceComponent);
    component = fixture.componentInstance;
    fixture.detectChanges();
  });

  it('should create', () => {
    expect(component).toBeTruthy();
  });
});
